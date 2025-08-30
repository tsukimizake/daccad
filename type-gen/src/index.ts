import { Project, InterfaceDeclaration, TypeAliasDeclaration, ClassDeclaration, EnumDeclaration, Type, Node, SyntaxKind, StringLiteral, SourceFile } from "ts-morph";
import * as fs from "fs";
import * as path from "path";
import { fileURLToPath } from "url";

// ESM equivalent of __dirname
const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

// Intermediate representation for Rust types
type RustTypeIR =
  | { kind: "primitive"; name: "i32" | "f64" | "bool" | "String" | "()" }
  | { kind: "array"; element: RustTypeIR; size?: number } // [T; N] or Vec<T>
  | { kind: "tuple"; elements: RustTypeIR[] }
  | { kind: "union"; variants: RustTypeIR[] }
  | { kind: "named"; name: string }
  | { kind: "generic"; base: string; args: RustTypeIR[] }
  | { kind: "option"; inner: RustTypeIR }
  | { kind: "js_value" };

// Method signature for wasm_bindgen
type Method = {
  name: string;
  args: Array<{ name: string; type: RustTypeIR; optional?: boolean }>;
  returnType: RustTypeIR;
  methodType: "constructor" | "static" | "instance";
};

// Intermediate representation for Rust items
type RustItem =
  | { kind: "struct"; name: string; fields: Array<{ name: string, type: RustTypeIR, optional?: boolean }>; derives?: string[] }
  | { kind: "enum"; name: string; variants: string[]; derives?: string[] }
  | { kind: "extern_block"; name: string; methods: Method[] }
  | { kind: "type_alias"; name: string; target: RustTypeIR };

function createProject(): Project {
  return new Project({
    compilerOptions: {
      target: 99, // Latest
      module: 1, // CommonJS
      strict: true,
    },
  });
}

type FilePrefix = 'Global' | 'Encapsulated' | 'Manifold';

function getFilePrefix(filePath: string): FilePrefix {
  const fileName = path.basename(filePath, '.d.ts');
  if (fileName.includes('manifold-global-types')) {
    return 'Global';
  } else if (fileName.includes('manifold-encapsulated-types')) {
    return 'Encapsulated';
  } else if (fileName.includes('manifold')) {
    return 'Manifold';
  }
  throw new Error(`Unknown file: ${filePath}`);
}

async function generateRustTypes(): Promise<void> {
  const project = createProject();
  generatedItems = new Map<string, RustItem>();

  // Load manifold-3d type definition files with absolute paths
  const manifoldPath = path.resolve(__dirname, "../../node_modules/manifold-3d");
  const dtsFiles = [
    path.join(manifoldPath, "manifold-global-types.d.ts"),
    path.join(manifoldPath, "manifold-encapsulated-types.d.ts"),
    path.join(manifoldPath, "manifold.d.ts")
  ];

  console.log("Loading manifold types from:", manifoldPath);

  for (const filePath of dtsFiles) {
    console.log("Checking file:", filePath);
    if (fs.existsSync(filePath)) {
      console.log("Adding file:", filePath);
      project.addSourceFileAtPath(filePath);
    } else {
      console.log("File not found:", filePath);
    }
  }

  // First pass: Register all type aliases from all files
  for (const sourceFile of project.getSourceFiles()) {
    registerTypeAliases(sourceFile);
  }

  // Second pass: Process all source files
  for (const sourceFile of project.getSourceFiles()) {
    console.log(`Processing file: ${sourceFile.getFilePath()}`);

    // Process interfaces
    const interfaces = sourceFile.getInterfaces();
    console.log(`Found ${interfaces.length} interfaces`);
    for (const interfaceDecl of interfaces) {
      const rustItem = processInterface(interfaceDecl);
      if (rustItem) {
        console.log(`Generated interface item: ${rustItem.name}, kind: ${rustItem.kind}`);
        const key = `${sourceFile.getBaseName()}-${rustItem.name}`;
        generatedItems.set(key, rustItem);
      } else {
        console.log(`Failed to process interface: ${interfaceDecl.getName()}`);
      }
    }

    // Process type aliases
    const typeAliases = sourceFile.getTypeAliases();
    console.log(`Found ${typeAliases.length} type aliases`);
    for (const typeAlias of typeAliases) {
      const rustItem = processTypeAlias(typeAlias);
      if (rustItem) {
        console.log(`Generated type alias item: ${rustItem.name}, kind: ${rustItem.kind}`);
        const key = `${sourceFile.getBaseName()}-${rustItem.name}`;
        generatedItems.set(key, rustItem);
      } else {
        console.log(`Failed to process type alias: ${typeAlias.getName()}`);
      }
    }

    // Process classes
    const classes = sourceFile.getClasses();
    console.log(`Found ${classes.length} classes`);
    for (const classDecl of classes) {
      const rustItem = processClass(classDecl);
      if (rustItem) {
        console.log(`Generated class item: ${rustItem.name}, kind: ${rustItem.kind}`);
        const key = `${sourceFile.getBaseName()}-${rustItem.name}`;
        generatedItems.set(key, rustItem);
      } else {
        console.log(`Failed to process class: ${classDecl.getName()}`);
      }
    }

    // Process enums
    const enums = sourceFile.getEnums();
    console.log(`Found ${enums.length} enums`);
    for (const enumDecl of enums) {
      const rustItem = processEnum(enumDecl);
      if (rustItem) {
        console.log(`Generated enum item: ${rustItem.name}, kind: ${rustItem.kind}`);
        const key = `${sourceFile.getBaseName()}-${rustItem.name}`;
        generatedItems.set(key, rustItem);
      } else {
        console.log(`Failed to process enum: ${enumDecl.getName()}`);
      }
    }
  }

  // Generate Rust output files (one per source file)
  generateRustOutputFiles(generatedItems);
}

function processInterface(interfaceDecl: InterfaceDeclaration): RustItem | null {
  const name = interfaceDecl.getName();
  if (!name) return null;

  const sourceFile = interfaceDecl.getSourceFile();
  const fileName = getFilePrefix(sourceFile.getFilePath());
  const properties = interfaceDecl.getProperties();

  // Special handling for SealedUint32Array and SealedFloat32Array
  if (name === "SealedUint32Array") {
    return {
      kind: "type_alias" as const,
      name,
      target: { kind: "named" as const, name: "[u32; N]" } // Generic const parameter
    };
  }

  if (name === "SealedFloat32Array") {
    return {
      kind: "type_alias" as const,
      name,
      target: { kind: "named" as const, name: "[f32; N]" } // Generic const parameter
    };
  }

  // Regular interfaces become structs using IR
  const fields = properties.map(prop => ({
    name: convertToSnakeCase(prop.getName()),
    type: convertTypeToRustIR(prop.getType()),
    optional: prop.hasQuestionToken()
  }));

  return {
    kind: "struct" as const,
    name,
    fields,
    derives: ["Debug", "Clone", "Serialize", "Deserialize"]
  };
}

function processTypeAlias(typeAlias: TypeAliasDeclaration): RustItem | null {
  const name = typeAlias.getName();
  const sourceFile = typeAlias.getSourceFile();
  const fileName = getFilePrefix(sourceFile.getFilePath());
  const aliasType = typeAlias.getType();

  // Handle special types like Vec2, Vec3 - these should not be prefixed
  if (name === "Vec2") {
    return {
      kind: "type_alias" as const,
      name: "Vec2", // No prefix for special types
      target: { kind: "array", element: { kind: "primitive", name: "f64" }, size: 2 }
    };
  } else if (name === "Vec3") {
    return {
      kind: "type_alias" as const,
      name: "Vec3", // No prefix for special types
      target: { kind: "array", element: { kind: "primitive", name: "f64" }, size: 3 }
    };
  } else if (name === "Mat3") {
    return {
      kind: "type_alias" as const,
      name: "Mat3", // No prefix for special types
      target: { kind: "array", element: { kind: "primitive", name: "f64" }, size: 9 }
    };
  } else if (name === "Mat4") {
    return {
      kind: "type_alias" as const,
      name: "Mat4", // No prefix for special types
      target: { kind: "array", element: { kind: "primitive", name: "f64" }, size: 16 }
    };
  } else if (name === "SimplePolygon") {
    return {
      kind: "type_alias" as const,
      name: "SimplePolygon", // No prefix for special types
      target: { kind: "array", element: { kind: "named", name: "Vec2" } }
    };
  } else if (name === "Polygons") {
    return {
      kind: "type_alias" as const,
      name: "Polygons", // No prefix for special types
      target: { kind: "array", element: { kind: "named", name: "SimplePolygon" } }
    };
  } else {
    // Use IR-based type conversion
    const convertedTypeIR = convertTypeToRustIR(aliasType);

    // Only encapsulated types from manifold.d.ts should be converted to JSValue
    if (fileName === "Manifold" && convertedTypeIR.kind === "named" && convertedTypeIR.name === name) {
      console.log(`Converting manifold.d.ts type alias to JSValue: ${name}`);
      return {
        kind: "type_alias" as const,
        name: name, // Keep original name for encapsulated types
        target: { kind: "js_value" as const }
      };
    } else {
      return {
        kind: "type_alias" as const,
        name,
        target: convertedTypeIR
      };
    }
  }
}

function processClass(classDecl: ClassDeclaration): RustItem | null {
  const name = classDecl.getName();
  if (!name) return null;

  const sourceFile = classDecl.getSourceFile();
  const fileName = getFilePrefix(sourceFile.getFilePath());

  const constructors = classDecl.getConstructors();
  const methods = classDecl.getMethods();
  const staticMethods = classDecl.getStaticMethods();
  const properties = classDecl.getProperties();

  // Check if this is an encapsulated type (from manifold-encapsulated-types.d.ts)
  const isEncapsulatedType = fileName === 'Encapsulated';

  if (isEncapsulatedType) {
    // For encapsulated types, generate wasm_bindgen extern "C" blocks using IR
    const methodsIR: Method[] = [];

    // Process constructors
    for (const constructor of constructors) {
      const params = constructor.getParameters();
      const args = params.map(param => {
        const paramTypeIR = convertTypeToRustIR(param.getType());
        console.log(`Constructor parameter ${param.getName()}: ${param.getType().getText()} -> ${rustTypeIRToString(paramTypeIR)}`);
        return {
          name: convertToSnakeCase(param.getName()),
          type: paramTypeIR
        };
      });

      methodsIR.push({
        name: "new",
        args,
        returnType: { kind: "named", name },
        methodType: "constructor"
      });
    }

    // Process static methods
    for (const method of staticMethods) {
      const methodName = convertToSnakeCase(method.getName());
      const params = method.getParameters();
      const returnTypeIR = convertTypeToRustIR(method.getReturnType());

      const args = params.map(param => ({
        name: convertToSnakeCase(param.getName()),
        type: convertTypeToRustIR(param.getType())
      }));

      methodsIR.push({
        name: methodName,
        args,
        returnType: returnTypeIR,
        methodType: "static"
      });
    }

    // Process instance methods
    for (const method of methods) {
      const methodName = convertToSnakeCase(method.getName());
      const params = method.getParameters();
      const returnTypeIR = convertTypeToRustIR(method.getReturnType());

      const args = params.map(param => ({
        name: convertToSnakeCase(param.getName()),
        type: convertTypeToRustIR(param.getType())
      }));

      methodsIR.push({
        name: methodName,
        args,
        returnType: returnTypeIR,
        methodType: "instance"
      });
    }

    return { kind: "extern_block", name, methods: methodsIR };
  } else if (properties.length > 0) {
    const fields = properties.map(prop => ({
      name: convertToSnakeCase(prop.getName()),
      type: convertTypeToRustIR(prop.getType()),
      optional: prop.hasQuestionToken()
    }));

    return {
      kind: "struct" as const,
      name,
      fields,
      derives: ["Debug", "Clone", "Serialize", "Deserialize"]
    };
  } else {
    // For other classes that are more like opaque handles
    let rustStruct = `// ${name} is an opaque type\n`;
    rustStruct += `#[derive(Debug, Serialize, Deserialize)]\npub struct ${name} {\n`;
    rustStruct += `    // This is an opaque handle\n`;
    rustStruct += `    _private: std::marker::PhantomData<()>,\n`;
    rustStruct += "}\n\n";

    return {
      kind: "type_alias" as const,
      name,
      target: { kind: "js_value" as const }
    };
  }
}

function processEnum(enumDecl: EnumDeclaration): RustItem | null {
  const name = enumDecl.getName();
  const sourceFile = enumDecl.getSourceFile();
  const fileName = getFilePrefix(sourceFile.getFilePath());
  const members = enumDecl.getMembers();

  let rustEnum = `#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]\npub enum ${name} {\n`;

  for (const member of members) {
    const memberName = member.getName();
    rustEnum += `    ${convertToPascalCase(memberName)},\n`;
  }

  rustEnum += "}\n\n";

  const variants = members.map(member => member.getName());
  return {
    kind: "enum" as const,
    name,
    variants,
    derives: ["Debug", "Clone", "Copy", "PartialEq", "Eq", "Serialize", "Deserialize"]
  };
}

// ==== TYPE CONVERSION FUNCTIONS ====

// Convert TypeScript Type to Rust IR
function convertTypeToRustIR(type: Type | undefined): RustTypeIR {
  if (!type) return { kind: "primitive", name: "()" };

  const typeText = type.getText().trim();

  // Handle primitive types
  const primitiveResult = convertPrimitiveToIR(typeText);
  if (primitiveResult) return primitiveResult;

  // Handle arrays and tuples
  if (type.isArray()) {
    const elementType = type.getArrayElementType();
    if (elementType) {
      return { kind: "array", element: convertTypeToRustIR(elementType) };
    }
  }

  // Handle tuple types like [number, number, number]
  if (type.isTuple()) {
    const elements = type.getTupleElements();
    const rustElements = elements.map(el => convertTypeToRustIR(el));

    // Check if all elements are the same primitive type
    if (rustElements.length > 0 &&
      rustElements.every(el => el.kind === "primitive") &&
      rustElements.every(el =>
        el.kind === "primitive" && rustElements[0].kind === "primitive" &&
        el.name === rustElements[0].name
      )
    ) {
      // Convert to fixed-size array [T; N]
      return { kind: "array", element: rustElements[0], size: rustElements.length };
    }

    return { kind: "tuple", elements: rustElements };
  }

  // Handle union types
  if (type.isUnion()) {
    // Try to preserve original type names from the text representation
    const originalText = typeText;

    // Check if the union contains known type alias names
    const unionParts = originalText.split('|').map(part => part.trim().replace(/import\([^)]*\)\./g, ''));
    const hasKnownAliases = unionParts.some(part => typeAliasRegistry.has(part));

    if (hasKnownAliases) {
      // Use original names for better readability
      const variants = unionParts.map(part => {
        if (typeAliasRegistry.has(part)) {
          return { kind: "named" as const, name: part };
        }
        // Try to convert the part
        const trimmedPart = part.trim();
        const primitiveResult = convertPrimitiveToIR(trimmedPart);
        if (primitiveResult) return primitiveResult;
        return { kind: "named" as const, name: trimmedPart };
      });

      // Check for Option<T> pattern (T | undefined)
      const undefinedVariant = variants.find(v => v.kind === "primitive" && v.name === "()");
      if (undefinedVariant && variants.length === 2) {
        const innerType = variants.find(v => v !== undefinedVariant);
        if (innerType) {
          return { kind: "option", inner: innerType };
        }
      }

      return { kind: "union", variants };
    }

    // Fallback to type-based analysis
    const variants = type.getUnionTypes().map(t => convertTypeToRustIR(t));

    // Check for Option<T> pattern (T | undefined)
    const undefinedVariant = variants.find(v => v.kind === "primitive" && v.name === "()");
    if (undefinedVariant && variants.length === 2) {
      const innerType = variants.find(v => v !== undefinedVariant);
      if (innerType) {
        return { kind: "option", inner: innerType };
      }
    }

    return { kind: "union", variants };
  }

  // Handle typeof expressions
  if (typeText.startsWith("typeof ")) {
    let referencedName = typeText.replace("typeof ", "").trim();
    // Clean import paths from typeof references
    referencedName = referencedName.replace(/import\([^)]*\)\./g, '');
    // For typeof expressions, we usually want the type itself, not a "typeof" wrapper
    // This handles cases like "typeof CrossSection" -> "CrossSection"
    return { kind: "named", name: referencedName };
  }

  // Handle function types like "() => void"
  if (typeText.includes("=>") || typeText.match(/^\(\s*\)\s*=>/)) {
    // Simple function type parsing - could be enhanced later
    if (typeText.includes("() => void")) {
      return { kind: "named", name: "fn()" };
    }
    // For now, use a generic function type
    return { kind: "named", name: "fn()" };
  }

  // Handle named types
  let cleanedName = typeText.replace(/import\([^)]*\)\./g, '').trim();

  // Handle import statements that might be lingering
  if (cleanedName.startsWith('import(')) {
    // Extract the actual type name from complex import expressions
    const match = cleanedName.match(/import\([^)]*\)\.(.+)$/);
    if (match) {
      cleanedName = match[1];
    }
  }

  // Check if this is a known type alias name
  for (const [aliasName, aliasDefinition] of typeAliasRegistry) {
    if (cleanedName === aliasName || cleanedName.endsWith(`.${aliasName}`)) {
      return { kind: "named", name: aliasName };
    }
  }

  return { kind: "named", name: cleanedName };
}

function convertPrimitiveToIR(typeText: string): RustTypeIR | null {
  switch (typeText) {
    case "string": return { kind: "primitive", name: "String" };
    case "number": return { kind: "primitive", name: "f64" };
    case "boolean": return { kind: "primitive", name: "bool" };
    case "void": return { kind: "primitive", name: "()" };
    case "undefined": return { kind: "primitive", name: "()" };
    default: return null;
  }
}

// Global type alias registry
const typeAliasRegistry = new Map<string, string>();

// Global generated items registry
let generatedItems: Map<string, RustItem>;

// Global todo types registry
const todoTypes = new Map<string, RustTypeIR[]>();

function registerTypeAliases(sourceFile: SourceFile) {
  const typeAliases = sourceFile.getTypeAliases();
  for (const alias of typeAliases) {
    const name = alias.getName();
    const typeNode = alias.getTypeNode();
    const typeText = typeNode?.getText();

    if (typeText) {
      // Fix known problematic type definitions
      if (name === "Polygons" && typeText.includes("SimplePolygon|SimplePolygon[]")) {
        // Fix Polygons to be just SimplePolygon[] to avoid API confusion
        typeAliasRegistry.set(name, "SimplePolygon[]");
        console.log(`Fixed Polygons type definition: ${name} = SimplePolygon[]`);
      } else {
        typeAliasRegistry.set(name, typeText);
      }

      // Check if this type alias should be converted to enum using AST
      if (typeNode && Node.isUnionTypeNode(typeNode)) {
        const unionTypes = typeNode.getTypeNodes();
        const stringLiterals = unionTypes
          .filter(t => Node.isLiteralTypeNode(t))
          .map(t => t.asKindOrThrow(SyntaxKind.LiteralType))
          .filter(t => Node.isStringLiteral(t.getLiteral()))
          .map(t => (t.getLiteral() as StringLiteral).getLiteralValue());

        // If all union types are string literals, create enum
        if (stringLiterals.length === unionTypes.length && stringLiterals.length > 1) {
          createEnumFromStringLiterals(sourceFile, name, stringLiterals);
        }
      }
    }
  }
}

function createEnumFromStringLiterals(sourceFile: SourceFile, name: string, stringLiterals: string[]) {
  // Convert string literals to PascalCase variants
  const variants = stringLiterals.map(literal => toPascalCase(literal));

  // Register as enum in our generated items
  const fileName = getFilePrefix(sourceFile.getFilePath());

  const enumItem: RustItem = {
    kind: "enum",
    name,
    variants,
    derives: ["Debug", "Clone", "Serialize", "Deserialize"]
  };

  // Add to generated items
  const key = `${sourceFile.getBaseName()}-${name}`;
  generatedItems.set(key, enumItem);

  console.log(`Created enum ${name} with variants: ${variants.join(', ')}`);
}

function toPascalCase(str: string): string {
  // Handle camelCase or compound words like "EvenOdd", "NonZero"
  // Already in proper PascalCase? Just return it
  if (/^[A-Z][a-z]*([A-Z][a-z]*)*$/.test(str)) {
    return str;
  }

  // Split on capital letters and common separators
  return str
    .replace(/([a-z])([A-Z])/g, '$1 $2') // Insert space before capital letters
    .split(/[\s_-]+/) // Split on spaces, underscores, hyphens
    .map(word => word.charAt(0).toUpperCase() + word.slice(1).toLowerCase())
    .join('');
}

// Global counter for sequential Todo Union IDs
let todoUnionCounter = 1;

// Cache for deduplicating Todo Unions by content
const todoUnionCache = new Map<string, string>(); // variants signature -> type name

function getNextTodoUnionId(): string {
  return (todoUnionCounter++).toString().padStart(3, '0');
}

function getOrCreateTodoUnionName(variants: RustTypeIR[]): string {
  // Create a signature for the variants to identify duplicates
  const signature = variants
    .map(v => JSON.stringify(v))
    .sort() // Sort to ensure consistent signature regardless of order
    .join('|');

  // Check if we already have a Todo Union for these variants
  if (todoUnionCache.has(signature)) {
    return todoUnionCache.get(signature)!;
  }

  // Create new Todo Union name
  const todoTypeName = `Todo${getNextTodoUnionId()}Union`;
  todoUnionCache.set(signature, todoTypeName);

  // Register this as a todo type to be generated
  registerTodoType(todoTypeName, variants);

  return todoTypeName;
}

function registerTodoType(typeName: string, variants: RustTypeIR[]) {
  if (!todoTypes.has(typeName)) {
    todoTypes.set(typeName, variants);
    console.log(`Registered todo union type: ${typeName} with ${variants.length} variants`);
  }
}

function generateTodoUnionStruct(typeName: string, variants: RustTypeIR[]): string {
  const variantsComment = variants.map(rustTypeIRToString).join(" | ");

  let result = `// TODO: Implement proper union type for: ${variantsComment}\n`;
  result += `// This is a placeholder struct - implement proper sum type or enum\n`;
  result += `#[derive(Debug, Clone, Serialize, Deserialize)]\n`;
  result += `pub struct ${typeName} {\n`;
  result += `    // TODO: Replace with proper union implementation\n`;
  result += `    // Possible variants: ${variantsComment}\n`;
  result += `    pub todo_data: String, // Placeholder - implement actual data structure\n`;
  result += `}\n\n`;

  result += `impl ${typeName} {\n`;
  result += `    pub fn todo() -> Self {\n`;
  result += `        Self {\n`;
  result += `            todo_data: "TODO: Implement union type".to_string()\n`;
  result += `        }\n`;
  result += `    }\n`;
  result += `}\n\n`;

  return result;
}

// Legacy function for backward compatibility during refactoring
function convertTypeToRust(type: Type | undefined): string {
  if (!type) return "()";

  let typeText = type.getText();
  if (!typeText) return "()";

  // Clean up the type text to remove import paths and extra spaces
  typeText = typeText.replace(/import\([^)]*\)\./g, '').trim();

  // Handle primitive types (exact conversion)
  const primitiveResult = convertPrimitiveTypes(typeText);
  if (primitiveResult) return primitiveResult;

  // Handle arrays (exact conversion)
  const arrayResult = convertArrayTypes(type, typeText);
  if (arrayResult) {
    console.log(`Array conversion: ${typeText} -> ${arrayResult}`);
    return arrayResult;
  }

  // Handle specific manifold types (exact conversion)
  const manifoldResult = convertManifoldTypes(typeText);
  if (manifoldResult) return manifoldResult;

  // Handle generics (exact conversion where possible)
  const genericResult = convertGenericTypes(typeText);
  if (genericResult) return genericResult;

  // Handle typeof expressions (exact conversion)
  const typeofResult = convertTypeofExpressions(typeText);
  if (typeofResult) return typeofResult;

  // Clean common TypeScript constructs
  typeText = cleanTypeScriptConstructs(typeText);

  // ==== SIMPLIFIED CONVERSIONS (TODO: implement properly) ====

  // SIMPLIFIED: Union types
  const unionResult = simplifyUnionTypes(typeText);
  if (unionResult) return unionResult;

  // SIMPLIFIED: Function types
  const functionResult = simplifyFunctionTypes(typeText);
  if (functionResult) return functionResult;

  // Default to the cleaned type name
  return typeText;
}

// ==== EXACT CONVERSION FUNCTIONS ====

function convertPrimitiveTypes(typeText: string): string | null {
  const trimmed = typeText.trim();
  switch (trimmed) {
    case "string": return "String";
    case "number": return "f64";
    case "boolean": return "bool";
    case "void": return "()";
    default: return null;
  }
}

function convertArrayTypes(type: Type, typeText: string): string | null {
  // Handle normal arrays like string[], number[]
  if (typeText.includes("[]")) {
    const elementType = type.getArrayElementType();
    if (elementType) {
      return `Vec<${convertTypeToRust(elementType)}>`;
    }
    return "Vec<()>";
  }

  // Handle tuple/fixed arrays like [number, number] or [number, number, number]
  if (typeText.startsWith("[") && typeText.endsWith("]")) {
    const inner = typeText.slice(1, -1);
    const elements = inner.split(",").map(s => s.trim());

    // Check if all elements are the same type
    const firstElementType = elements[0];
    const allSameType = elements.every(el => el === firstElementType);

    if (allSameType) {
      // Convert to fixed-size array [T; N] - recursively convert the element type
      const convertedElementType = convertPrimitiveTypes(firstElementType);
      if (convertedElementType) {
        return `[${convertedElementType}; ${elements.length}]`;
      }
    }

    // For mixed types or unknown types, fall back to Vec - recursively convert first element type
    const convertedFirstType = convertPrimitiveTypes(firstElementType) || "String";
    return `Vec<${convertedFirstType}>`;
  }

  return null;
}

function convertManifoldTypes(typeText: string): string | null {
  switch (typeText) {
    case "Readonly<Vec2>":
    case "Vec2": return "Vec2";
    case "Readonly<Vec3>":
    case "Vec3": return "Vec3";
    case "Float32Array": return "Vec<f32>";
    case "Uint32Array": return "Vec<u32>";
    default: return null;
  }
}

function convertGenericTypes(typeText: string): string | null {
  if (typeText.includes("<") && typeText.includes(">")) {
    const baseType = typeText.split('<')[0];
    if (baseType === "SealedUint32Array") return "Vec<u32>";
    if (baseType === "SealedFloat32Array") return "Vec<f32>";
    if (baseType === "readonly") {
      // Handle readonly arrays
      const innerType = typeText.match(/readonly\s*\(([^)]+)\)\[]/);
      if (innerType) {
        return `Vec<${convertTypeToRust(undefined)}>`;
      }
    }
    return baseType;
  }
  return null;
}

function convertTypeofExpressions(typeText: string): string | null {
  if (typeText.startsWith("typeof ")) {
    return typeText.replace("typeof ", "");
  }
  return null;
}

function cleanTypeScriptConstructs(typeText: string): string {
  typeText = typeText.replace(/\| undefined/g, '');
  typeText = typeText.replace(/undefined \|/g, '');
  typeText = typeText.replace(/readonly /g, '');
  typeText = typeText.replace(/Readonly</g, '<');
  return typeText;
}

// ==== SIMPLIFIED CONVERSION FUNCTIONS (TODO: implement properly) ====

function simplifyUnionTypes(typeText: string): string | null {
  // SIMPLIFIED: Union types with string literals (like FillRule, JoinType, ErrorStatus)
  if (typeText.includes("|") && typeText.includes("'")) {
    // TODO: Extract enum-like values and create proper Rust enum
    return "String"; // Simplified for now
  }

  // SIMPLIFIED: General union types (e.g., "Polygons | CrossSection")
  if (typeText.includes("|")) {
    const types = typeText.split("|").map(t => t.trim());
    // For now, just use the first type or create an enum-like representation
    if (types.length === 2 && (types.includes("number") || types.includes("boolean") || types.includes("string"))) {
      // Handle simple union with primitives
      return types[0] === "number" ? "f64" : types[0] === "boolean" ? "bool" : "String";
    }
    // TODO: For complex types, create proper sum types
    return `/* Union: ${typeText} */ String`; // Placeholder
  }

  return null;
}

function simplifyFunctionTypes(typeText: string): string | null {
  // SIMPLIFIED: Function types
  if (typeText.includes("=>") || typeText.includes("() =>")) {
    // TODO: Parse function signatures properly
    return "fn()"; // Simplified function type
  }
  return null;
}

function convertToSnakeCase(str: string): string {
  return str.replace(/([A-Z])/g, "_$1").toLowerCase().replace(/^_/, "");
}

function convertToPascalCase(str: string): string {
  return str.charAt(0).toUpperCase() + str.slice(1);
}

// ==== CODE GENERATION FUNCTIONS ====

// Convert RustTypeIR to string
function rustTypeIRToString(typeIR: RustTypeIR): string {
  switch (typeIR.kind) {
    case "primitive":
      return typeIR.name;
    case "array":
      const elementStr = rustTypeIRToString(typeIR.element);
      return typeIR.size
        ? `[${elementStr}; ${typeIR.size}]`
        : `Vec<${elementStr}>`;
    case "tuple":
      const elementsStr = typeIR.elements.map(rustTypeIRToString).join(", ");
      return `(${elementsStr})`;
    case "union":
      // Use deduplicated Todo Union name
      return getOrCreateTodoUnionName(typeIR.variants);
    case "named":
      return typeIR.name;
    case "generic":
      const argsStr = typeIR.args.map(rustTypeIRToString).join(", ");
      return `${typeIR.base}<${argsStr}>`;
    case "option":
      return `Option<${rustTypeIRToString(typeIR.inner)}>`;
    case "js_value":
      return "wasm_bindgen::JsValue";
    default:
      return "()";
  }
}

// Generate Rust code for RustItem
function rustItemToString(item: RustItem): string {
  switch (item.kind) {
    case "struct":
      const derives = item.derives ? `#[derive(${item.derives.join(", ")})]\n` : "";
      let structCode = `${derives}pub struct ${item.name} {\n`;
      for (const field of item.fields) {
        const fieldType = field.optional
          ? `Option<${rustTypeIRToString(field.type)}>`
          : rustTypeIRToString(field.type);
        structCode += `    pub ${field.name}: ${fieldType},\n`;
      }
      structCode += "}\n\n";
      return structCode;

    case "enum":
      const enumDerives = item.derives ? `#[derive(${item.derives.join(", ")})]\n` : "";
      let enumCode = `${enumDerives}pub enum ${item.name} {\n`;

      // For Global enums, add explicit string values
      const isGlobalEnum = item.name.startsWith("Global");

      for (const variant of item.variants) {
        if (isGlobalEnum) {
          enumCode += `    ${variant} = "${variant}",\n`;
        } else {
          enumCode += `    ${variant},\n`;
        }
      }
      enumCode += "}\n\n";
      return enumCode;

    case "extern_block":
      let externCode = `#[wasm_bindgen]\nextern "C" {\n`;
      externCode += `    type ${item.name};\n\n`;

      for (const method of item.methods) {
        if (method.methodType === "constructor") {
          externCode += `    #[wasm_bindgen(constructor)]\n`;
          externCode += `    fn new(`;
        } else if (method.methodType === "static") {
          externCode += `    #[wasm_bindgen(static_method_of = ${item.name}, js_name = ${method.name})]\n`;
          externCode += `    fn ${method.name}(`;
        } else {
          externCode += `    #[wasm_bindgen(method)]\n`;
          externCode += `    fn ${method.name}(this: &${item.name}`;
          if (method.args.length > 0) externCode += ", ";
        }

        const argStrings = method.args.map(arg => {
          const argType = arg.optional
            ? `Option<${rustTypeIRToString(arg.type)}>`
            : rustTypeIRToString(arg.type);
          return `${arg.name}: ${argType}`;
        });

        externCode += argStrings.join(", ");
        externCode += `) -> ${rustTypeIRToString(method.returnType)};\n\n`;
      }

      externCode += "}\n\n";
      return externCode;

    case "type_alias":
      return `pub type ${item.name} = ${rustTypeIRToString(item.target)};\n\n`;

    default:
      return "";
  }
}

function generateRustOutputFiles(generatedItems: Map<string, RustItem>): void {
  const outputDir = path.resolve(__dirname, "../../generated-rust-types");
  if (!fs.existsSync(outputDir)) {
    fs.mkdirSync(outputDir, { recursive: true });
  }

  // Group items by source file
  const fileGroups = new Map<string, RustItem[]>();
  
  for (const [key, rustItem] of generatedItems) {
    // Key format is based on getBaseName() which removes .d.ts extension
    let fileName: string;
    
    if (key.includes("manifold-global-types")) {
      fileName = "manifold-global-types.d.ts";
    } else if (key.includes("manifold-encapsulated-types")) {
      fileName = "manifold-encapsulated-types.d.ts";
    } else if (key.startsWith("manifold.d.ts-")) {
      fileName = "manifold.d.ts";
    } else {
      fileName = "unknown";
    }
    
    if (!fileGroups.has(fileName)) {
      fileGroups.set(fileName, []);
    }
    fileGroups.get(fileName)!.push(rustItem);
  }

  // Generate separate file for each source file
  for (const [fileName, items] of fileGroups) {
    generateSingleRustFile(fileName, items, outputDir);
  }

  // Generate Todo Unions in a separate shared file
  generateTodoUnionsFile(outputDir);

  console.log(`Generated Rust types written to ${fileGroups.size} files`);
  console.log(`Generated ${generatedItems.size} items and ${todoTypes.size} todo union types`);
}

function generateSingleRustFile(fileName: string, items: RustItem[], outputDir: string): void {
  let rustOutput = `// Auto-generated Rust types from ${fileName}\n\n`;

  // Check what imports we need based on these items
  const allCode = items.map(rustItemToString).join("");
  const needsWasmBindgen = allCode.includes("wasm_bindgen");
  const needsSerde = allCode.includes("Serialize") || allCode.includes("Deserialize");

  // Add necessary imports
  if (needsWasmBindgen) {
    rustOutput += `use wasm_bindgen::prelude::*;\n`;
  }

  if (needsSerde) {
    rustOutput += `use serde::{Serialize, Deserialize};\n`;
  }

  // Add common imports only if needed
  if (allCode.includes("HashMap")) {
    rustOutput += `use std::collections::HashMap;\n`;
  }
  if (allCode.includes("Debug")) {
    rustOutput += `use std::fmt::Debug;\n`;
  }

  // Import Todo Unions if referenced
  if (allCode.includes("Todo")) {
    rustOutput += `use super::todo_unions::*;\n`;
  }

  rustOutput += "\n";

  // Add all items for this file
  for (const rustItem of items) {
    rustOutput += rustItemToString(rustItem);
  }

  // Generate file name mapping
  const outputFileName = getOutputFileName(fileName);
  const outputPath = path.join(outputDir, outputFileName);
  fs.writeFileSync(outputPath, rustOutput);

  console.log(`Generated ${outputFileName} with ${items.length} items`);
}

function generateTodoUnionsFile(outputDir: string): void {
  if (todoTypes.size === 0) return;

  let rustOutput = "// Auto-generated Todo Union types\n\n";
  rustOutput += `use serde::{Serialize, Deserialize};\n\n`;

  // Add todo union types as structs
  for (const [typeName, variants] of todoTypes) {
    rustOutput += generateTodoUnionStruct(typeName, variants);
  }

  const outputPath = path.join(outputDir, "todo_unions.rs");
  fs.writeFileSync(outputPath, rustOutput);

  console.log(`Generated todo_unions.rs with ${todoTypes.size} todo union types`);
}

function getOutputFileName(sourceFileName: string): string {
  switch (sourceFileName) {
    case "manifold-global-types.d.ts":
      return "global_types.rs";
    case "manifold-encapsulated-types.d.ts":
      return "encapsulated_types.rs";
    case "manifold.d.ts":
      return "manifold_main.rs";
    default:
      return `${sourceFileName.replace('.d.ts', '')}.rs`;
  }
}

// Main execution
async function main() {
  try {
    await generateRustTypes();
    console.log("Type generation completed successfully!");
  } catch (error) {
    console.error("Error during type generation:", error);
    process.exit(1);
  }
}

// ESM equivalent of require.main === module check
if (import.meta.url === `file://${process.argv[1]}`) {
  main();
}

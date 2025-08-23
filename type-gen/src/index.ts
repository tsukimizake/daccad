import { Project, InterfaceDeclaration, TypeAliasDeclaration, ClassDeclaration, EnumDeclaration, Type } from "ts-morph";
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
  ret: RustTypeIR;
  is_static?: boolean;
  is_constructor?: boolean;
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
  const generatedItems = new Map<string, RustItem>();

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

  // Process all source files
  for (const sourceFile of project.getSourceFiles()) {
    console.log(`Processing file: ${sourceFile.getFilePath()}`);

    // Process interfaces
    const interfaces = sourceFile.getInterfaces();
    console.log(`Found ${interfaces.length} interfaces`);
    // TODO: Convert to IR approach
    // For now, skip interfaces to focus on classes
    // for (const interfaceDecl of interfaces) {
    //   const rustType = processInterface(interfaceDecl);
    //   if (rustType) {
    //     const key = `${sourceFile.getBaseName()}-${rustType.name}`;
    //     generatedItems.set(key, rustType);
    //   }
    // }

    // Process type aliases
    const typeAliases = sourceFile.getTypeAliases();
    console.log(`Found ${typeAliases.length} type aliases`);
    // TODO: Convert to IR approach
    // For now, skip type aliases to focus on classes
    // for (const typeAlias of typeAliases) {
    //   const rustType = processTypeAlias(typeAlias);
    //   if (rustType) {
    //     const key = `${sourceFile.getBaseName()}-${rustType.name}`;
    //     generatedItems.set(key, rustType);
    //   }
    // }

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

    // TODO: Convert to IR approach  
    // Process enums
    const enums = sourceFile.getEnums();
    console.log(`Found ${enums.length} enums`);
    // for (const enumDecl of enums) {
    //   const rustType = processEnum(enumDecl);
    //   if (rustType) {
    //     const key = `${sourceFile.getBaseName()}-${rustType.name}`;
    //     generatedItems.set(key, rustType);
    //   }
    // }
  }

  // Generate Rust output file
  generateRustOutput(generatedItems);
}

function processInterface(interfaceDecl: InterfaceDeclaration): RustType | null {
  const name = interfaceDecl.getName();
  const sourceFile = interfaceDecl.getSourceFile();
  const fileName = getFilePrefix(sourceFile.getFilePath());
  const prefixedName = `${fileName}${name}`;
  const properties = interfaceDecl.getProperties();
  const methods = interfaceDecl.getMethods();

  // Special handling for SealedUint32Array and SealedFloat32Array
  if (name === "SealedUint32Array") {
    const rustStruct = `// Fixed-size array type for ${name}\n` +
      `pub type ${name}<const N: usize> = [u32; N];\n\n`;
    return { kind: "sealed_array", name: prefixedName, rustCode: rustStruct };
  }

  if (name === "SealedFloat32Array") {
    const rustStruct = `// Fixed-size array type for ${name}\n` +
      `pub type ${name}<const N: usize> = [f32; N];\n\n`;
    return { kind: "sealed_array", name: prefixedName, rustCode: rustStruct };
  }

  let rustStruct = `#[derive(Debug, Clone, Serialize, Deserialize)]\npub struct ${prefixedName} {\n`;

  // Process properties
  for (const prop of properties) {
    const propName = prop.getName();
    const propType = convertTypeToRust(prop.getType());
    const isOptional = prop.hasQuestionToken();

    if (isOptional) {
      rustStruct += `    pub ${convertToSnakeCase(propName)}: Option<${propType}>,\n`;
    } else {
      rustStruct += `    pub ${convertToSnakeCase(propName)}: ${propType},\n`;
    }
  }

  rustStruct += "}\n\n";

  // Generate implementation with methods
  if (methods.length > 0) {
    rustStruct += `impl ${prefixedName} {\n`;

    for (const method of methods) {
      const methodName = method.getName();
      const params = method.getParameters();
      const returnType = convertTypeToRust(method.getReturnType());

      let methodSignature = `    pub fn ${convertToSnakeCase(methodName)}(`;
      methodSignature += "&self";

      for (const param of params) {
        const paramName = param.getName();
        const paramType = convertTypeToRust(param.getType());
        const isOptional = param.hasQuestionToken();

        if (isOptional) {
          methodSignature += `, ${convertToSnakeCase(paramName)}: Option<${paramType}>`;
        } else {
          methodSignature += `, ${convertToSnakeCase(paramName)}: ${paramType}`;
        }
      }

      methodSignature += `) -> ${returnType} {\n`;
      methodSignature += `        todo!("Implement ${methodName}")\n`;
      methodSignature += "    }\n\n";

      rustStruct += methodSignature;
    }

    rustStruct += "}\n\n";
  }

  return { kind: "struct", name: prefixedName, rustCode: rustStruct };
}

function processTypeAlias(typeAlias: TypeAliasDeclaration): RustType | null {
  const name = typeAlias.getName();
  const sourceFile = typeAlias.getSourceFile();
  const fileName = getFilePrefix(sourceFile.getFilePath());
  const prefixedName = `${fileName}${name}`;
  const aliasType = typeAlias.getType();


  // Handle special types like Vec2, Vec3 - these should not be prefixed
  if (name === "Vec2") {
    const rustType = "pub type Vec2 = [f64; 2];\n\n";
    const kind = "vec2";
    return { kind, name, rustCode: rustType };
  } else if (name === "Vec3") {
    const rustType = "pub type Vec3 = [f64; 3];\n\n";
    const kind = "vec3";
    return { kind, name, rustCode: rustType };
  } else if (name === "Mat3") {
    const rustType = "pub type Mat3 = [f64; 9];\n\n";
    const kind = "matrix";
    return { kind, name, rustCode: rustType };
  } else if (name === "Mat4") {
    const rustType = "pub type Mat4 = [f64; 16];\n\n";
    const kind = "matrix";
    return { kind, name, rustCode: rustType };
  } else if (name === "SimplePolygon") {
    const rustType = "pub type SimplePolygon = Vec<Vec2>;\n\n";
    const kind = "polygon";
    return { kind, name, rustCode: rustType };
  } else if (name === "Polygons") {
    const rustType = "pub type Polygons = Vec<SimplePolygon>;\n\n";
    const kind = "polygon";
    return { kind, name, rustCode: rustType };
  } else {
    const convertedType = convertTypeToRust(aliasType);

    // Only encapsulated types should be converted to JSValue
    if (fileName === "Manifold" && convertedType === name) {
      console.log(`Converting manifold.d.ts type alias to JSValue: ${name}`);
      const rustType = `// ${name} - encapsulated type represented as JSValue\npub type ${name} = wasm_bindgen::JsValue;\n\n`;
      const kind = "encapsulated";
      return { kind, name, rustCode: rustType };
    } else {
      const rustType = `pub type ${prefixedName} = ${convertedType};\n\n`;
      const kind = "type_alias";
      return { kind, name: prefixedName, rustCode: rustType };
    }
  }
}

function processClass(classDecl: ClassDeclaration): RustItem | null {
  const name = classDecl.getName();
  if (!name) return null;

  const sourceFile = classDecl.getSourceFile();
  const fileName = getFilePrefix(sourceFile.getFilePath());
  const prefixedName = `${fileName}${name}`;

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
    let rustStruct = `#[derive(Debug, Clone, Serialize, Deserialize)]\npub struct ${prefixedName} {\n`;

    for (const prop of properties) {
      const propName = prop.getName();
      const propType = convertTypeToRust(prop.getType());
      rustStruct += `    pub ${convertToSnakeCase(propName)}: ${propType},\n`;
    }

    rustStruct += "}\n\n";

    // Generate implementation
    rustStruct += `impl ${prefixedName} {\n`;

    // Process constructors
    for (const constructor of constructors) {
      const params = constructor.getParameters();
      let constructorSignature = "    pub fn new(";

      for (let i = 0; i < params.length; i++) {
        const param = params[i];
        const paramName = param.getName();
        const paramType = convertTypeToRust(param.getType());
        const isOptional = param.hasQuestionToken();

        if (i > 0) constructorSignature += ", ";

        if (isOptional) {
          constructorSignature += `${convertToSnakeCase(paramName)}: Option<${paramType}>`;
        } else {
          constructorSignature += `${convertToSnakeCase(paramName)}: ${paramType}`;
        }
      }

      constructorSignature += `) -> Self {\n`;
      constructorSignature += `        todo!("Implement constructor")\n`;
      constructorSignature += "    }\n\n";

      rustStruct += constructorSignature;
    }

    // Process static methods
    for (const method of staticMethods) {
      const methodName = method.getName();
      const params = method.getParameters();
      const returnType = convertTypeToRust(method.getReturnType());

      let methodSignature = `    pub fn ${convertToSnakeCase(methodName)}(`;

      for (let i = 0; i < params.length; i++) {
        const param = params[i];
        const paramName = param.getName();
        const paramType = convertTypeToRust(param.getType());
        const isOptional = param.hasQuestionToken();

        if (i > 0) methodSignature += ", ";

        if (isOptional) {
          methodSignature += `${convertToSnakeCase(paramName)}: Option<${paramType}>`;
        } else {
          methodSignature += `${convertToSnakeCase(paramName)}: ${paramType}`;
        }
      }

      methodSignature += `) -> ${returnType} {\n`;
      methodSignature += `        todo!("Implement ${methodName}")\n`;
      methodSignature += "    }\n\n";

      rustStruct += methodSignature;
    }

    // Process instance methods
    for (const method of methods) {
      const methodName = method.getName();
      const params = method.getParameters();
      const returnType = convertTypeToRust(method.getReturnType());

      let methodSignature = `    pub fn ${convertToSnakeCase(methodName)}(`;
      methodSignature += "&self";

      for (const param of params) {
        const paramName = param.getName();
        const paramType = convertTypeToRust(param.getType());
        const isOptional = param.hasQuestionToken();

        if (isOptional) {
          methodSignature += `, ${convertToSnakeCase(paramName)}: Option<${paramType}>`;
        } else {
          methodSignature += `, ${convertToSnakeCase(paramName)}: ${paramType}`;
        }
      }

      methodSignature += `) -> ${returnType} {\n`;
      methodSignature += `        todo!("Implement ${methodName}")\n`;
      methodSignature += "    }\n\n";

      rustStruct += methodSignature;
    }

    rustStruct += "}\n\n";

    return { kind: "struct", name: prefixedName, rustCode: rustStruct };
  } else {
    // For other classes that are more like opaque handles
    let rustStruct = `// ${prefixedName} is an opaque type\n`;
    rustStruct += `#[derive(Debug, Serialize, Deserialize)]\npub struct ${prefixedName} {\n`;
    rustStruct += `    // This is an opaque handle\n`;
    rustStruct += `    _private: std::marker::PhantomData<()>,\n`;
    rustStruct += "}\n\n";

    return { kind: "opaque", name: prefixedName, rustCode: rustStruct };
  }
}

function processEnum(enumDecl: EnumDeclaration): RustType | null {
  const name = enumDecl.getName();
  const sourceFile = enumDecl.getSourceFile();
  const fileName = getFilePrefix(sourceFile.getFilePath());
  const prefixedName = `${fileName}${name}`;
  const members = enumDecl.getMembers();

  let rustEnum = `#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]\npub enum ${prefixedName} {\n`;

  for (const member of members) {
    const memberName = member.getName();
    rustEnum += `    ${convertToPascalCase(memberName)},\n`;
  }

  rustEnum += "}\n\n";

  return { kind: "enum", name: prefixedName, rustCode: rustEnum };
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
    if (rustElements.length > 0 && rustElements.every(el => 
      el.kind === "primitive" && el.name === rustElements[0].name && rustElements[0].kind === "primitive"
    )) {
      // Convert to fixed-size array [T; N]
      return { kind: "array", element: rustElements[0], size: rustElements.length };
    }
    
    return { kind: "tuple", elements: rustElements };
  }

  // Handle union types
  if (type.isUnion()) {
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

  // Handle named types
  const cleanedName = typeText.replace(/import\([^)]*\)\./g, '').trim();
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
      // For now, represent as comments. TODO: implement proper sum types
      const variantsStr = typeIR.variants.map(rustTypeIRToString).join(" | ");
      return `/* Union: ${variantsStr} */ String`;
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
      for (const variant of item.variants) {
        enumCode += `    ${variant},\n`;
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

function generateRustOutput(generatedItems: Map<string, RustItem>): void {
  const outputDir = path.resolve(__dirname, "../../generated-rust-types");
  if (!fs.existsSync(outputDir)) {
    fs.mkdirSync(outputDir, { recursive: true });
  }

  let rustOutput = "// Auto-generated Rust types from manifold-3d TypeScript definitions\n\n";

  // Check what imports we need based on generated items
  const allCode = Array.from(generatedItems.values()).map(rustItemToString).join("");
  const needsWasmBindgen = allCode.includes("wasm_bindgen");
  const needsSerde = allCode.includes("Serialize") || allCode.includes("Deserialize");

  // Add necessary imports
  if (needsWasmBindgen) {
    rustOutput += `use wasm_bindgen::prelude::*;\n`;
  }

  if (needsSerde) {
    rustOutput += `use serde::{Serialize, Deserialize};\n`;
  }

  // Add common imports
  rustOutput += `use std::collections::HashMap;\n`;
  rustOutput += `use std::fmt::Debug;\n`;

  rustOutput += "\n";

  // Add all generated items
  for (const [_, rustItem] of generatedItems) {
    rustOutput += rustItemToString(rustItem);
  }

  const outputPath = path.join(outputDir, "manifold_types.rs");
  fs.writeFileSync(outputPath, rustOutput);

  console.log(`Generated Rust types written to: ${outputPath}`);
  console.log(`Generated ${generatedItems.size} items`);
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

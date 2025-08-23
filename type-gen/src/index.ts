import { Project, InterfaceDeclaration, TypeAliasDeclaration, ClassDeclaration, EnumDeclaration, Type } from "ts-morph";
import * as fs from "fs";
import * as path from "path";

// Union type for different kinds of Rust types
type RustType =
  | { kind: "struct"; name: string; rustCode: string; }
  | { kind: "enum"; name: string; rustCode: string; }
  | { kind: "encapsulated"; name: string; rustCode: string; }
  | { kind: "type_alias"; name: string; rustCode: string; }
  | { kind: "opaque"; name: string; rustCode: string; }
  | { kind: "vec2"; name: string; rustCode: string; }
  | { kind: "vec3"; name: string; rustCode: string; }
  | { kind: "matrix"; name: string; rustCode: string; }
  | { kind: "sealed_array"; name: string; rustCode: string; }
  | { kind: "polygon"; name: string; rustCode: string; };

function createProject(): Project {
  return new Project({
    compilerOptions: {
      target: 99, // Latest
      module: 1, // CommonJS
      strict: true,
    },
  });
}

async function generateRustTypes(): Promise<void> {
  const project = createProject();
  const generatedTypes = new Map<string, RustType>();

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
    for (const interfaceDecl of interfaces) {
      const rustType = processInterface(interfaceDecl);
      if (rustType) {
        generatedTypes.set(rustType.name, rustType);
      }
    }

    // Process type aliases
    const typeAliases = sourceFile.getTypeAliases();
    console.log(`Found ${typeAliases.length} type aliases`);
    for (const typeAlias of typeAliases) {
      const rustType = processTypeAlias(typeAlias);
      if (rustType) {
        generatedTypes.set(rustType.name, rustType);
      }
    }

    // Process classes
    const classes = sourceFile.getClasses();
    console.log(`Found ${classes.length} classes`);
    for (const classDecl of classes) {
      const rustType = processClass(classDecl);
      if (rustType) {
        generatedTypes.set(rustType.name, rustType);
      }
    }

    // Process enums
    const enums = sourceFile.getEnums();
    console.log(`Found ${enums.length} enums`);
    for (const enumDecl of enums) {
      const rustType = processEnum(enumDecl);
      if (rustType) {
        generatedTypes.set(rustType.name, rustType);
      }
    }
  }

  // Generate Rust output file
  generateRustOutput(generatedTypes);
}

function processInterface(interfaceDecl: InterfaceDeclaration): RustType | null {
  const name = interfaceDecl.getName();
  const properties = interfaceDecl.getProperties();
  const methods = interfaceDecl.getMethods();

  // Special handling for SealedUint32Array and SealedFloat32Array
  if (name === "SealedUint32Array") {
    const rustStruct = `// Fixed-size array type for ${name}\n` +
      `pub type ${name}<const N: usize> = [u32; N];\n\n`;
    return { kind: "sealed_array", name, rustCode: rustStruct };
  }

  if (name === "SealedFloat32Array") {
    const rustStruct = `// Fixed-size array type for ${name}\n` +
      `pub type ${name}<const N: usize> = [f32; N];\n\n`;
    return { kind: "sealed_array", name, rustCode: rustStruct };
  }

  let rustStruct = `#[derive(Debug, Clone, Serialize, Deserialize)]\npub struct ${name} {\n`;

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
    rustStruct += `impl ${name} {\n`;

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

  return { kind: "struct", name, rustCode: rustStruct };
}

function processTypeAlias(typeAlias: TypeAliasDeclaration): RustType | null {
  const name = typeAlias.getName();
  const aliasType = typeAlias.getType();

  let rustType: string;
  let kind: RustType["kind"];

  // Handle tuple types like Vec2, Vec3
  if (name === "Vec2") {
    rustType = "pub type Vec2 = [f64; 2];\n\n";
    kind = "vec2";
  } else if (name === "Vec3") {
    rustType = "pub type Vec3 = [f64; 3];\n\n";
    kind = "vec3";
  } else if (name === "Mat3") {
    rustType = "pub type Mat3 = [f64; 9];\n\n";
    kind = "matrix";
  } else if (name === "Mat4") {
    rustType = "pub type Mat4 = [f64; 16];\n\n";
    kind = "matrix";
  } else if (name === "SimplePolygon") {
    rustType = "pub type SimplePolygon = Vec<Vec2>;\n\n";
    kind = "polygon";
  } else if (name === "Polygons") {
    rustType = "pub type Polygons = Vec<SimplePolygon>;\n\n";
    kind = "polygon";
  } else {
    const convertedType = convertTypeToRust(aliasType);

    // Skip self-referencing types like "pub type CrossSection = CrossSection;"
    if (convertedType === name) {
      console.log(`Skipping self-referencing type alias: ${name}`);
      return null;
    }

    rustType = `pub type ${name} = ${convertedType};\n\n`;
    kind = "type_alias";
  }

  return { kind, name, rustCode: rustType };
}

function processClass(classDecl: ClassDeclaration): RustType | null {
  const name = classDecl.getName();
  if (!name) return null;

  const constructors = classDecl.getConstructors();
  const methods = classDecl.getMethods();
  const staticMethods = classDecl.getStaticMethods();
  const properties = classDecl.getProperties();

  // Check if this is an encapsulated type (from manifold-encapsulated-types.d.ts)
  const sourceFile = classDecl.getSourceFile();
  const isEncapsulatedType = sourceFile.getFilePath().includes('manifold-encapsulated-types.d.ts');

  // Generate struct with properties if any
  let rustStruct: string;

  if (isEncapsulatedType) {
    // For encapsulated types like CrossSection, Manifold, Mesh - use JSValue
    rustStruct = `// ${name} from manifold-encapsulated-types - represented as JSValue\n`;
    rustStruct += `pub type ${name} = wasm_bindgen::JsValue;\n\n`;

    // For JSValue types, we don't generate impl blocks
    return { kind: "encapsulated", name, rustCode: rustStruct };
  } else if (properties.length > 0) {
    rustStruct = `#[derive(Debug, Clone, Serialize, Deserialize)]\npub struct ${name} {\n`;

    for (const prop of properties) {
      const propName = prop.getName();
      const propType = convertTypeToRust(prop.getType());
      rustStruct += `    pub ${convertToSnakeCase(propName)}: ${propType},\n`;
    }

    rustStruct += "}\n\n";

    // For structs with properties, we generate impl blocks later
    // Generate implementation - this will be followed by impl block generation
  } else {
    // For other classes that are more like opaque handles
    rustStruct = `// ${name} is an opaque type\n`;
    rustStruct += `#[derive(Debug, Serialize, Deserialize)]\npub struct ${name} {\n`;
    rustStruct += `    // This is an opaque handle\n`;
    rustStruct += `    _private: std::marker::PhantomData<()>,\n`;
    rustStruct += "}\n\n";
  }

  // Generate implementation
  rustStruct += `impl ${name} {\n`;

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

  // Determine if this is a struct or opaque type
  const kind = properties.length > 0 ? "struct" : "opaque";
  return { kind, name, rustCode: rustStruct };
}

function processEnum(enumDecl: EnumDeclaration): RustType | null {
  const name = enumDecl.getName();
  const members = enumDecl.getMembers();

  let rustEnum = `#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]\npub enum ${name} {\n`;

  for (const member of members) {
    const memberName = member.getName();
    rustEnum += `    ${convertToPascalCase(memberName)},\n`;
  }

  rustEnum += "}\n\n";

  return { kind: "enum", name, rustCode: rustEnum };
}

// ==== TYPE CONVERSION FUNCTIONS ====

function convertTypeToRust(type: Type | undefined): string {
  if (!type) return "()";

  let typeText = type.getText();
  if (!typeText) return "()";

  // Clean up the type text to remove import paths
  typeText = typeText.replace(/import\([^)]*\)\./g, '');

  // Handle primitive types (exact conversion)
  const primitiveResult = convertPrimitiveTypes(typeText);
  if (primitiveResult) return primitiveResult;

  // Handle arrays (exact conversion)
  const arrayResult = convertArrayTypes(type, typeText);
  if (arrayResult) return arrayResult;

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
  switch (typeText) {
    case "string": return "String";
    case "number": return "f64";
    case "boolean": return "bool";
    case "void": return "()";
    default: return null;
  }
}

function convertArrayTypes(type: Type, typeText: string): string | null {
  if (typeText.includes("[]")) {
    const elementType = type.getArrayElementType();
    if (elementType) {
      return `Vec<${convertTypeToRust(elementType)}>`;
    }
    return "Vec<()>";
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

function generateRustOutput(generatedTypes: Map<string, RustType>): void {
  const outputDir = path.resolve(__dirname, "../../generated-rust-types");
  if (!fs.existsSync(outputDir)) {
    fs.mkdirSync(outputDir, { recursive: true });
  }

  let rustOutput = "// Auto-generated Rust types from manifold-3d TypeScript definitions\n\n";

  // Check what imports we need based on generated types
  const needsWasmBindgen = Array.from(generatedTypes.values()).some(
    type => type.rustCode.includes("wasm_bindgen::JsValue")
  );

  const needsSerde = Array.from(generatedTypes.values()).some(
    type => type.rustCode.includes("Serialize") || type.rustCode.includes("Deserialize")
  );

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

  // Add all generated types
  for (const [_, rustType] of generatedTypes) {
    rustOutput += rustType.rustCode;
  }

  const outputPath = path.join(outputDir, "manifold_types.rs");
  fs.writeFileSync(outputPath, rustOutput);

  console.log(`Generated Rust types written to: ${outputPath}`);
  console.log(`Generated ${generatedTypes.size} types`);
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

if (require.main === module) {
  main();
}

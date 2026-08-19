# Code Practices for AI Agents

## General guidance
Before writing any code, read all relevant files and understand the existing patterns - your changes must blend in with the surrounding codebase.
If the change is large or introduces a new pattern, **PAUSE and ask the user for confirmation** before proceeding.
When a task requires changes, present your proposed solution first or ask for clarification before proceeding.
If you believe the request cannot be fulfilled, explain why.
Don’t default to hacky workarounds just to force a result.

1. Avoid use of en dash or em dash, use hyphen.
2. Never manually execute `git` or `gh` commands without confirmation.
3. Don't create random scripts unless explicitly asked.
4. Don't try to run the program or run tests unless explicitly asked.
5. When asked to verify a code path read the code and analyze instead of executing the program.
6. Use clean, descriptive, self-explanatory function and variable names (prefer camelCase). Watch out for JSON fields which are usually snake_case.
7. Prefer native tooling for reading, writing, editing files rather than using `cat` or `sed`.
8. After completing a bug fix or feature implementation, suggest a concise commit message the user could use.
9. Never introduce new dependencies without explicit user approval. If a dependency is required, justify its necessity over standard library solutions.

## Extract repetitive logic used 3 or more times
```rs
// Bad: repeats the same logic in multiple branches, making the differences hard to spot.
let r: u8 = vals
    .get(0)
    .ok_or("color requires <r>")?
    .parse()
    .map_err(|e: std::num::ParseIntError| format!("invalid r: {e}"))?;
let g: u8 = vals
    .get(1)
    .ok_or("color requires <g>")?
    .parse()
    .map_err(|e: std::num::ParseIntError| format!("invalid g: {e}"))?;
let b: u8 = vals
    .get(2)
    .ok_or("color requires <b>")?
    .parse()
    .map_err(|e: std::num::ParseIntError| format!("invalid b: {e}"))?;
    
    
// Good: extracts the shared logic into one function, so the code is easier to understand at a glance and maintain.
fn parse_rgb_component(vals: &[String], index: usize, name: &str) -> Result<u8, String> {
    let str = match vals.get(index) {
        Some(s) => s,
        None => return Err(format!("color requires <{name}>")),
    };
    str.parse::<u8>().map_err(|e| format!("invalid {name}: {e}"))
}
let r = parse_rgb_component(&vals, 0, "r")?;
let g = parse_rgb_component(&vals, 1, "g")?;
let b = parse_rgb_component(&vals, 2, "b")?;
```

## Extract Condensed Construction Logic
**Rule**:
If constructor initialization or update block exceeds three lines or mixes creation logic with conditional updates,
extract it into a dedicated helper function. That makes it clear what set of objects a block of code relies on.
Massive inline constructors obscure intent and force readers to parse irrelevant implementation details.
```rs
// ❌ Bad - Condensed logic, mixed creation and update, hard to read at a glance
if settings.lighting.is_none() {
    settings.lighting = Some(LightingSettings { 
        effect: Effect::Static, 
        color: [r, g, b], 
        brightness: 255 
    });
} else if let Some(ref mut l) = settings.lighting {
    l.color = [r, g, b];
}


// ✅ Good - Intent is clear, construction details are encapsulated
update_lighting_color(&mut settings, [r, g, b])

// Helper function
fn update_lighting_color(settings: &mut DeviceSettings, color: [u8; 3]) {
    match &mut settings.lighting {
        Some(lighting) => {
            lighting.color = color;
        }
        None => {
            settings.lighting = Some(LightingSettings {
                effect: Effect::Static,
                color,
                brightness: 255,
            });
        }
    }
}
```

## Don't chain more than two functions with lambdas
Code is data. Code should document itself. Prefer code that makes the intent obvious at a glance.
```rs
// Bad - hidden intent, collects just to immediately transform again
let list = pids.iter().map(|p| format!("{p}")).collect::<Vec<_>>().join(", ");

// ✅ Good
let list = format_list(pids);

// Also good - short, direct, easy to read
let ok = str.chars().all(|c| c.is_ascii_alphanumeric() || c == ' ' || c == '-' || c == '_');
```

## Use let-else with Option<T> where appropriate
```rs
// Bad - only two functions are chained but an empty lambda is used to satisfy the function argument which is awkward and unnecessarily complicated
let definition = registry
        .find_by_pid(pid)
        .ok_or_else(|| format!("PID {pid:#06x} missing"))?;

// Good - much cleaner
let Some(definition) = registry.find_by_pid(pid) else {
    return Err(format!("PID {pid:#06x} missing"));
};
```
This improves readability by making returns explicit, so it’s clear where the function exits instead of hiding that control flow behind `?`

## Preserve meaningful names in display output
> Keep user-facing values readable and semantically correct.
> Don’t anonymize or flatten display parameters when a named variable or computed value makes the output clearer.
> Prefer descriptive variables to inline expressions that hide intent.
```rs
// Do
let mouse_name = &definition.name;
let percentage = value as u32 * 100 / 255;
println!(
    "{mouse_name}: set brightness {value}/255 ({percentage}%) on LED {led:#04x}",
);

// Don't
println!(
    "{}: set brightness {}/255 ({}%) on LED {led:#04x}",
    definition.name, value, value / 255
);
```


## Watch for overflow in multiplication when generating code
> Be careful with integer multiplication: operands may need to be widened before multiplying to avoid overflow in the original type. 
> Prefer casting to a larger type first when the result can exceed the input type’s range.

```rs
// Bad: overflows in the original type
let num: u8 = 20;
let result = num * 15;

// Good: widen before multiplying
let num: u8 = 20;
let result = num as u32 * 15;
```

## Don’t add complexity if the data structure is wrong.
Don’t force the idea into an existing structure if that structure is a poor fit; 
refactor to match the goal instead of layering complexity on top of incompatibility.

```rs
// Bad: uses Vec + sort + dedup just to count unique IDs
let mut ids = Vec::new();
for product in products {
    ids.push(product.id);
}
ids.sort();
ids.dedup();

match ids.len() {
    0 => Err("no products found"),
    1 => Some("only a single product found"),
    _ => Some("multiple products found"),
}
```

```rs
// Good: use a set when uniqueness is the actual goal
use std::collections::HashSet;

let mut ids = HashSet::new();
for product in products {
    ids.insert(product.id);
}

match ids.len() {
    0 => Err("no products found"),
    1 => Some("only a single product found"),
    _ => Some("multiple products found"),
}
```

## Avoid replacing a few simple if statements with iterator tricks
For a small fixed set of booleans, prefer straightforward if pushes over list based processing.
```rs
// Avoid
let features: Vec<&str> = [
  premium_package.cooling.then_some("cooling"),
  premium_package.power.then_some("power"),
  premium_package.warranty.then_some("warranty"),
  premium_package.battery.then_some("battery"),
]
.into_iter()
.flatten()
.collect();
```
This increases overall code complexity because `flatten()` is doing something less obvious here: 
it removes `None` from `Option<T>` values, not just nested collections.

```rs
// Prefer
let mut features = Vec::new();
if premium_package.cooling { caps.push("cooling"); }
if premium_package.power { caps.push("power"); }
if premium_package.warranty { caps.push("warranty"); }
if premium_package.battery { caps.push("battery"); }
```

## Don't clutter if statement logic to force a oneliner
Avoid chaining multiple `Option`/`Result` transformations inside an if condition just to save lines. 
This increases cognitive load and makes debugging harder, hindering variable reuse during future code refactors. 
Instead, use let-else statements to extract values early to keep logic clear.
```rs
// ❌ Bad: Dense logic couples extraction, conversion, and comparison in one line.
// Hard to set breakpoints on individual steps; requires mental parsing of a chain.
if entry.path().extension().and_then(|e| e.to_str()) != Some("mp3") {
    continue;
}
```

```rs
// ✅ Good: relaxed statements, easy to read, debug, and modify.
let path = entry.path();
let Some(ext) = path.extension() else {
    continue
};
if ext != "mp3" {
    continue
}
```

## Prefer Robust Step-by-Step Logic Over Functional Conciseness
```rs
// Avoid (Overly functional) - harder to debug and read for simple validation
let x = values.first().ok_or("missing")?.parse::<u16>()
    .map_err(|e| format!("invalid x: {e}"))?;
    
// Preferred (Explicit & Safe)
if vals.is_empty() {
    return Err("missing".to_string());
}
let x: u16 = values[0].parse().map_err(|e| format!("invalid x: {e}"))?;
```
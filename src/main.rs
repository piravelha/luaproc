use std::{
  fs::File,
  io::{Write, Read},
  env,
  iter::zip,
  process::Command,
};
use regex::Regex;

#[derive(Debug, PartialEq, Clone)]
enum TokenKind {
  Name,
  MacroName,
  Property,
  Number,
  String,
  Special,
  Brace,
  Delimiter,
  DefineDirective,
  Ifdef,
  Ifndef,
  Endif,
  Undef,
  EndDefine,
  Newline,
  Stringify,
  Vararg,
  TaggedVararg,
  Paste,
}

#[derive(Debug, Clone)]
struct Token {
  kind: TokenKind,
  value: String,
}

#[derive(Debug, Clone)]
struct ValueMacro {
  name: String,
  value: Vec<Token>,
}

#[derive(Debug, Clone)]
struct FunctionMacro {
  name: String,
  params: Vec<String>,
  value: Vec<Token>,
}

fn lex_single_token(input: &String) -> Option<(String, Token)> {
  let regexes = vec![(
    Regex::new(r"^(\.|:)\s*[a-zA-Z_]\w*").unwrap(),
    TokenKind::Property,
  ), (
    Regex::new(r"^(\.\.\.)").unwrap(),
    TokenKind::Vararg,
  ), (
    Regex::new(r"^(#\.\.\.)").unwrap(),
    TokenKind::TaggedVararg,
  ), (
    Regex::new(r"^(#[a-zA-Z_]\w*#)").unwrap(),
    TokenKind::Stringify,
  ), (
    Regex::new(r"^(##)").unwrap(),
    TokenKind::Paste,
  ), (
    Regex::new(r"^[a-zA-Z_]\w*!").unwrap(),
    TokenKind::MacroName,
  ), (
    Regex::new(r"^[a-zA-Z_]\w*").unwrap(),
    TokenKind::Name,
  ), (
    Regex::new(r"^-?\d+(\.\d+)?").unwrap(),
    TokenKind::Number,
  ), (
    Regex::new(r#"^"([^"\\]|\\.)*""#).unwrap(),
    TokenKind::String,
  ), (
    Regex::new(r"^#define").unwrap(),
    TokenKind::DefineDirective,
  ), (
    Regex::new(r"^#ifdef").unwrap(),
    TokenKind::Ifdef,
  ), (
    Regex::new(r"^#ifndef").unwrap(),
    TokenKind::Ifndef,
  ), (
    Regex::new(r"^#endif").unwrap(),
    TokenKind::Endif,
  ), (
    Regex::new(r"^#undef").unwrap(),
    TokenKind::Undef,
  ), (
    Regex::new(r"^#end").unwrap(),
    TokenKind::EndDefine,
  ), (
    Regex::new(r"^[+\-*/!@#$%^&:=~<>?.]+").unwrap(),
    TokenKind::Special,
  ), (
    Regex::new(r"^[()\[\]{}]").unwrap(),
    TokenKind::Brace,
  ), (
    Regex::new(r"^[,;]").unwrap(),
    TokenKind::Delimiter,
  ), (
    Regex::new(r"^(\r?\n[\t ]*)+").unwrap(),
    TokenKind::Newline,
  )];
  for (re, kind) in regexes {
    if let Some(m) = re.captures(input) {
      let full = &m[0];
      return Some((
        input[full.len()..].to_string(),
        Token { kind, value: full.to_string() },
      ));
    }
  }
  None
}

fn lex_whole_input(input: &String) -> Option<Vec<Token>> {
  let mut tokens: Vec<Token> = vec![];
  let mut remaining = input
    .trim_start_matches(|c| c == ' ' || c == '\t')
    .to_string();
  while !remaining.is_empty() {
    if let Some((rest, token)) = lex_single_token(&remaining) {
      tokens.push(token);
      remaining = rest
        .trim_start_matches(|c| c == ' ' || c == '\t')
        .to_string();
    } else {
      println!("Tokens: {:?}", tokens);
      return None;
    }
  }
  Some(tokens)
}

fn eval_pastes(tokens: &Vec<Token>) -> Vec<Token> {
  let mut raw_i = 0;
  let mut new_tokens = vec![];
  while raw_i < tokens.len() {
    raw_i += 1;
    let mut i = raw_i - 1;
    if tokens[i].kind == TokenKind::Name {
      let mut parts = vec![tokens[i].value.clone()];
      while i + 1 < tokens.len() {
        if tokens[i + 1].kind == TokenKind::Paste {
          if tokens.len() > i + 2 && tokens[i + 2].kind == TokenKind::Name {
            i += 1;
            parts.push(tokens[i + 1].value.clone());
          } else {
            break
          }
        } else { break }
        i += 1;
      }
      new_tokens.push(Token {
        kind: TokenKind::Name,
        value: parts.join(""),
      });
      raw_i = i + 1;
    } else {
      new_tokens.push(tokens[i].clone());
    }
  }
  new_tokens
}

fn get_macros(tokens: &Vec<Token>, value_macros: &mut Vec<ValueMacro>, func_macros: &mut Vec<FunctionMacro>) -> Vec<Token> {
  let mut i = 0;
  let mut new_tokens = vec![];
  while i < tokens.len() {
    i += 1;
    let token;
    match tokens.into_iter().nth(i - 1) {
      None => continue,
      Some(t) => token = t,
    }
    new_tokens.push(token.clone());
    if token.kind == TokenKind::Undef {
      if let Some(name) = tokens.into_iter().nth(i) {
        i += 1;
        new_tokens.pop();
        if name.kind != TokenKind::MacroName {
          continue;
        }
        let mut new_value_macros = vec![];
        for value_macro in value_macros.clone().into_iter() {
          if value_macro.name == name.value {
            continue;
          }
          new_value_macros.push(value_macro.clone());
        }
        value_macros.clear();
        value_macros.extend(new_value_macros);
        let mut new_func_macros = vec![];
        for func_macro in func_macros.clone().into_iter() {
          if func_macro.name == name.value {
            continue;
          }
          new_func_macros.push(func_macro.clone());
        }
        func_macros.clear();
        func_macros.extend(new_func_macros);
      }
    }
    for value_macro in value_macros.clone().into_iter() {
      match apply_value_macro_once(tokens.clone().into_iter().skip(i - 1).collect(), value_macro.clone()) {
        Some(result_tokens) => {
          new_tokens.pop();
          new_tokens.extend(eval_pastes(&get_macros(&result_tokens, value_macros, func_macros)));
          break;
        },
        None => {},
      }
    }
    for func_macro in func_macros.clone().into_iter() {
      match apply_func_macro_once(tokens.clone().into_iter().skip(i).collect(), func_macro) {
        Some((result_tokens, new_i)) => {
          new_tokens.extend(eval_pastes(&get_macros(&result_tokens, value_macros, func_macros)));
          i += new_i;
          break;
        },
        None => {},
      }
    } 
    if token.kind == TokenKind::Ifdef || token.kind == TokenKind::Ifndef {
      if let Some(var) = tokens.into_iter().nth(i) {
        new_tokens.pop();
        if var.kind != TokenKind::MacroName {
          continue;
        }
        let mut contains = false;
        for val_macro in value_macros.into_iter() {
          if val_macro.name == var.value {
            contains = true;
            break
          }
        }
        for func_macro in func_macros.into_iter() {
          if func_macro.name == var.value {
            contains = true;
            break
          }
        }
        if token.kind == TokenKind::Ifndef {
          contains = !contains;
        }
        if !contains {
          for tok in tokens.into_iter().skip(i + 1) {
            i += 1;
            if tok.kind == TokenKind::Endif {
              break
            }
          }
        }
        i += 1;
      }
      continue;
    }
    if token.kind == TokenKind::Endif {
      new_tokens.pop();
      continue;
    }
    if token.kind == TokenKind::DefineDirective {
      if let Some(name) = tokens.into_iter().nth(i) {
        if name.kind != TokenKind::MacroName {
          continue;
        }
        if let Some(eq) = tokens.into_iter().nth(i + 1) {
          if eq.value.as_str() == "(" {
            let params_iter = tokens.into_iter()
              .skip(i + 2)
              .take_while(|t| t.value.as_str() != ")")
              .inspect(|_| i += 1)
              .collect::<Vec<_>>();
            let params = params_iter
              .split(|t| t.kind == TokenKind::Delimiter);
            let mut all_names = true;
            let mut names = vec![];
            params.for_each(|sub| {
              if sub.len() > 1 {
                all_names = false;
              }
              if let Some(name) = sub.into_iter().nth(0) {
                names.push(name.value.clone());
              }
            });
            if let Some(eq) = tokens.into_iter().nth(i + 3) {
              if eq.value.as_str() != "=" {
                continue
              }
              let value: Vec<Token> = tokens.clone()
                .into_iter()
                .skip(i + 4)
                .take_while(|t|
                  t.kind != TokenKind::EndDefine)
                .collect();
              let define = FunctionMacro {
                name: name.value.clone(),
                params: names,
                value: value.clone(),
              };
              func_macros.push(define);
              i += 5 + value.len();
              new_tokens.pop();
            }
            continue
          }
          if eq.kind == TokenKind::Newline {
            value_macros.push(ValueMacro {
              name: name.value.clone(),
              value: vec![],
            });
            i += 2;
            new_tokens.pop();
          }
          if eq.value.as_str() != "=" {
            continue;
          }
          let value: Vec<Token> = tokens.clone()
            .into_iter()
            .skip(i + 2)
            .take_while(|t|
              t.kind != TokenKind::EndDefine)
            .collect();
          let define = ValueMacro {
            name: name.value.clone(),
            value: value.clone(),
          };
          value_macros.push(define);
          i += 3 + value.len();
          new_tokens.pop();
        }
      }
    } 
  }
  new_tokens
}

fn apply_value_macro_once(input: Vec<Token>, value_macro: ValueMacro) -> Option<Vec<Token>> {
  let token = input.clone().into_iter().nth(0)?;
  if token.kind == TokenKind::MacroName && token.value == value_macro.name {
    if input.len() > 1 && input[1].value.as_str() == "=" {
      return Some(vec![token])
    }
    return Some(value_macro.value.clone());
  } else if token.kind == TokenKind::Stringify && token.value[1..token.value.len()-1] == value_macro.name {
    let tok = Token {kind: TokenKind::String, value: format!("{:?}", render_tokens_as_string(value_macro.value.clone()))};
    return Some(vec![tok]);
  } else if token.kind == TokenKind::Delimiter && input.len() > 1 && (input[1].value.as_str() == "__VA_ARGS__" || input[1].value.as_str() == "#...") {
    if value_macro.value.len() != 0 {
      return Some(vec![token]);
    }
  } else if token.kind == TokenKind::TaggedVararg {
    return Some(value_macro.value.clone());
  }
  None
}

fn apply_value_macros(input: Vec<Token>, value_macro: ValueMacro) -> Vec<Token> {
  input.clone().into_iter().enumerate().flat_map(|(i, token)| {
    if token.kind == TokenKind::Name && token.value == value_macro.name {
      if i + 1 < input.len() && input[i + 1].value.as_str() == "=" {
        return vec![token];
      }
      value_macro.value.clone()
    } else if token.kind == TokenKind::Stringify && token.value[1..token.value.len()-1] == value_macro.name {
      let tok = Token {kind: TokenKind::String, value: format!("{:?}", render_tokens_as_string(value_macro.value.clone()))};
      vec![tok]
    } else if token.kind == TokenKind::Delimiter && i + 1 < input.len() && (input[i + 1].value.as_str() == "__VA_ARGS__" || input[i + 1].value.as_str() == "#...") {
      if value_macro.value.len() != 0 {
        return vec![token];
      }
      vec![]
    } else if token.kind == TokenKind::TaggedVararg {
      return value_macro.value.clone();
    } else {
      vec![token]
    }
  }).collect()
}

fn apply_func_macro_once(input: Vec<Token>, func_macro: FunctionMacro) -> Option<(Vec<Token>, usize)> {
  let tokens = input;
  let token = tokens.clone().into_iter().nth(0)?;
  if token.kind == TokenKind::MacroName && token.value == func_macro.name {
    if let Some(lparen) = tokens.clone().into_iter().nth(1) {
      let lparen_val = lparen.value.as_str();
      if lparen_val != "(" && lparen_val != "[" && lparen_val != "{" {
        return None;
      }
      let mut nesting_level = 0;
      let mut args: Vec<Vec<Token>> = vec![vec![]];
      let mut i = 0;
      for cur_token in tokens.clone().into_iter().skip(2) {
        i += 1;
        if cur_token.kind == TokenKind::Delimiter && nesting_level == 0 {
          args.push(vec![]);
          continue;
        }
        if cur_token.value.as_str() == "(" || cur_token.value.as_str() == "{" || cur_token.value.as_str() == "[" || cur_token.value.as_str() == "function" || cur_token.value.as_str() == "do" || cur_token.value.as_str() == "then" {
          nesting_level += 1;
        } else if cur_token.value.as_str() == ")" || cur_token.value.as_str() == "}" || cur_token.value.as_str() == "]" || cur_token.value.as_str() == "end" {
          nesting_level -= 1;
          if nesting_level == -1 {
            break;
          }
        }
        let l = args.clone().len();
        args[l - 1].push(cur_token);
      }
      let mut value = func_macro.value.clone();
      let mut k = 0;
      zip(func_macro.params.clone(), args.clone()).for_each(|(param, arg)| {
        let val_macro;
        if param.as_str() == "..." {
          let varargs = args.clone().into_iter().skip(k);
          let mut comma_sep = vec![];
          for (l, arg) in varargs.enumerate() {
            if l > 0 {
              comma_sep.push(Token {
                kind: TokenKind::Delimiter,
                value: ",".to_string(),
              });
            }
            comma_sep.extend(arg);
          }
          val_macro = ValueMacro {
            name: "__VA_ARGS__".to_string(),
            value: comma_sep,
          }
        } else {
          val_macro = ValueMacro {
            name: param,
            value: arg,
          };
        }
        value = apply_value_macros(value.clone(), val_macro);
        k += 1;
      });
      return Some((value.clone(), i + 2));
    }
  }
  None
}

fn render_tokens_as_string(tokens: Vec<Token>) -> String {
  tokens.into_iter()
    .map(|t| t.value.clone())
    .reduce(|acc, v| ({
      if acc.ends_with("\n") {
        acc + &v
      } else {
        acc + " " + &v
      }
    }).to_string())
    .unwrap()
}

fn main() {
  let args: Vec<_> = env::args().collect();
  if args.len() < 2 {
    println!("Usage: luaproc <filename>");
    return;
  }
  let file_path = args[1].clone();
  let file_res = File::open(file_path);
  let input;
  match file_res {
    Err(e) => {
      eprintln!("Could not open file: {}", e);
      return;
    },
    Ok(mut f) => {
      let mut content = String::new();
      match f.read_to_string(&mut content) {
        Err(e) => {
          eprintln!("Could not read from file: {}", e);
          return;
        },
        Ok(_) => input = content,
      }
    },
  }
  let backslash_re = Regex::new(r"\\\r?\n").unwrap();
  let input = backslash_re.replace_all(&input, "").to_string();
  let result = lex_whole_input(&input);
  let tokens;
  match result {
    None => {
      eprintln!("Tokenization Failed");
      return;
    },
    Some(ts) => tokens = ts
  }
  let tokens = get_macros(&tokens, &mut vec![], &mut vec![]);
  let result = render_tokens_as_string(tokens);
  let out = File::create("out.lua");
  match out {
    Err(e) => {
      eprintln!("Could not create out.lua: {}", e);
      return;
    },
    Ok(mut f) => {
      match f.write_all(result.as_bytes()) {
        Err(e) => eprintln!("Could not write content: {}", e),
        Ok(_) => {},
      }
    }
  }
  match Command::new("stylua")
    .arg("out.lua")
    .output() {
    Err(e) => {
      eprintln!("Could not format with stylua: {}", e);
      return;
    },
    Ok(_) => {},
  }
}


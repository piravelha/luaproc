use std::{
  fs::File,
  io::{Write, Read},
  env,
  iter::zip
};
use regex::Regex;

#[derive(Debug, PartialEq, Clone)]
enum TokenKind {
  Name,
  Property,
  Number,
  String,
  Special,
  Brace,
  Delimiter,
  DefineDirective,
  Newline,
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
    .trim_matches(|c| c == ' ' || c == '\t')
    .to_string();
  while !remaining.is_empty() {
    if let Some((rest, token)) = lex_single_token(&remaining) {
      tokens.push(token);
      remaining = rest
        .trim_matches(|c| c == ' ' || c == '\t')
        .to_string();
    } else {
      println!("Tokens: {:?}", tokens);
      return None;
    }
  }
  Some(tokens)
}

fn get_macros(tokens: &Vec<Token>) -> (Vec<Token>, Vec<ValueMacro>, Vec<FunctionMacro>) {
  let mut value_macros = vec![];
  let mut func_macros = vec![];
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
    if token.kind == TokenKind::DefineDirective {
      if let Some(name) = tokens.into_iter().nth(i) {
        if name.kind != TokenKind::Name {
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
                  t.kind != TokenKind::Newline)
                .collect();
              let define = FunctionMacro {
                name: name.value.clone(),
                params: names,
                value: value.clone(),
              };
              func_macros.push(define);
              i += 4 + value.len();
              new_tokens.pop();
            }
            continue
          }
          if eq.value.as_str() != "=" {
            continue;
          }
          let value: Vec<Token> = tokens.clone()
            .into_iter()
            .skip(i + 2)
            .take_while(|t|
              t.kind != TokenKind::Newline)
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
  (new_tokens, value_macros, func_macros)
}

fn apply_value_macro(input: Vec<Token>, value_macro: ValueMacro) -> Vec<Token> {
  input.clone().into_iter().enumerate().flat_map(|(i, token)| {
    if token.kind == TokenKind::Name && token.value == value_macro.name {
      if i < input.len() && input[i + 1].value.as_str() == "=" {
        return vec![token];
      }
      value_macro.value.clone()
    } else {
      vec![token]
    }
  }).collect()
}

fn apply_func_macro(input: Vec<Token>, func_macro: FunctionMacro) -> Vec<Token> {
  let tokens = input;
  let mut new_tokens = vec![];
  let mut i = 0;
  while i < tokens.clone().len() {
    i += 1;
    let token = tokens.clone().into_iter().nth(i - 1).unwrap();
    new_tokens.push(token.clone());
    if token.kind == TokenKind::Name && token.value == func_macro.name {
      if let Some(lparen) = tokens.clone().into_iter().nth(i) {
        if lparen.value.as_str() != "(" {
          continue
        }
        let mut nesting_level = 0;
        let mut j = i + 1;
        let mut args: Vec<Vec<Token>> = vec![vec![]];
        let mut args_idx = 0;
        while j < tokens.clone().len() {
          j += 1;
          i += 1;
          let cur_token = tokens.clone().into_iter().nth(j - 1).unwrap();
          if cur_token.kind == TokenKind::Delimiter && nesting_level == 0 {
            args.push(vec![]);
            args_idx += 1;
            continue;
          }
          if cur_token.value.as_str() == "(" {
            nesting_level += 1;
          } else if cur_token.value.as_str() == ")" {
            nesting_level -= 1;
            if nesting_level == -1 {
              break;
            }
          }
          args[args_idx].push(cur_token);
        }
        let mut value = func_macro.value.clone();
        zip(func_macro.params.clone(), args.clone()).for_each(|(param, arg)| {
          let val_macro = ValueMacro {
            name: param,
            value: arg,
          };
          value = apply_value_macro(value.clone(), val_macro);
        });
        new_tokens.pop();
        new_tokens.extend(value.clone());
        i += 1;
      
      }
    }
  }
  new_tokens 
}

fn render_tokens_as_string(tokens: Vec<Token>) -> String {
  tokens.into_iter()
    .map(|t| t.value.clone())
    .reduce(|acc, v| (acc + " " + &v).to_string())
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
  let backslash_re = Regex::new(r"\\\n").unwrap();
  let input = backslash_re.replace_all(&input, " ").to_string();
  let result = lex_whole_input(&input);
  let tokens;
  match result {
    None => {
      eprintln!("Tokenization Failed");
      return;
    },
    Some(ts) => tokens = ts
  }
  let (mut tokens, value_macros, func_macros) = get_macros(&tokens);
  value_macros
    .into_iter()
    .for_each(|m|
      tokens = apply_value_macro(tokens.clone(), m)
    );
  func_macros
    .into_iter()
    .for_each(|m|
      tokens = apply_func_macro(tokens.clone(), m)
    );
  let result = render_tokens_as_string(tokens);
  let ws_re = Regex::new(r" ([()\[\]{},;]+)").unwrap();
  let result = ws_re.replace_all(&result, "$1").to_string();
  let ws_re = Regex::new(r"([(\[{]+) ").unwrap();
  let result = ws_re.replace_all(&result, "$1").to_string();
  let out = File::create("out.lua");
  match out {
    Err(e) => {
      eprintln!("Could not create out.lua: {}", e);
      return;
    },
    Ok(mut f) => {
      match f.write_all(result.as_bytes()) {
        Err(e) => eprintln!("Could not write content: {}", e),
        Ok(_) => return,
      }
    }
  }
}


#!/usr/bin/env bash
# Generate the fixed multi-file corpus used by benchmark.md timings.
# Usage: scripts/gen_bench_corpus.sh <output_dir>
set -euo pipefail

OUT="${1:?usage: $0 <output_dir>}"
rm -rf "$OUT"
mkdir -p "$OUT"/{js,ts,json,py,rs,go,sh,md,yaml,toml,css,html,lua,rb,sql,xml,php,hcl,c}

for i in $(seq 1 150); do
	cat >"$OUT/js/file_$i.js" <<EOF
function hello$i(  name  ){
const x={a:1,b:2,c:3}
return  name+x.a
}
export  const  v$i  =  hello$i(  "world"  )
EOF
	cat >"$OUT/ts/file_$i.ts" <<EOF
interface User$i { name: string; age: number }
function greet$i(  u:User$i  ):string{
return  \`Hello \${u.name}\`
}
export  const  u$i:User$i={name:"a",age:$i}
EOF
	cat >"$OUT/json/file_$i.json" <<EOF
{"name":"item$i","values":[1,2,3],"nested":{"a":true,"b":false}}
EOF
	cat >"$OUT/py/file_$i.py" <<EOF
def hello_$i( name ):
    x = { "a": 1, "b": 2 }
    return name + str(x["a"])

class Foo$i:
    def __init__( self, v ):
        self.v = v
EOF
	cat >"$OUT/rs/file_$i.rs" <<EOF
pub fn hello_$i(name:&str)->String{
let x=vec![1,2,3];
format!("{}{}",name,x[0])
}
EOF
	cat >"$OUT/go/file_$i.go" <<EOF
package main
import "fmt"
func Hello$i(name string) string {
return fmt.Sprintf("%s%d",name,$i)
}
EOF
	cat >"$OUT/sh/file_$i.sh" <<EOF
#!/bin/bash
echo  "hello $i"
if  [  -n  "\$1"  ];  then
echo  "\$1"
fi
EOF
	cat >"$OUT/md/file_$i.md" <<EOF
# Title $i

Some **bold** and *italic* text.

- item 1
- item 2

\`\`\`js
const x = 1
\`\`\`
EOF
	cat >"$OUT/yaml/file_$i.yaml" <<EOF
name: item$i
values:
  - a
  - b
  - c
nested:
  enabled: true
  count: $i
EOF
	cat >"$OUT/toml/file_$i.toml" <<EOF
[package]
name = "item$i"
version = "0.1.0"

[deps]
foo = "1.0"
bar = { version = "2.0", optional = true }
EOF
	cat >"$OUT/css/file_$i.css" <<EOF
.foo$i{color:red;margin:0;padding:10px}
.bar$i  {  display:flex;  gap:  8px  }
EOF
	cat >"$OUT/html/file_$i.html" <<EOF
<!DOCTYPE html>
<html><head><title>Page $i</title></head>
<body><div  class="wrap"><h1>Hello</h1><p>text $i</p></div></body></html>
EOF
	cat >"$OUT/lua/file_$i.lua" <<EOF
local function hello_$i( name )
local x = { a = 1, b = 2 }
return name .. tostring(x.a)
end
return { hello = hello_$i }
EOF
	cat >"$OUT/rb/file_$i.rb" <<EOF
def hello_$i(name)
  x = { a: 1, b: 2 }
  "#{name}#{x[:a]}"
end

class Foo$i
  def initialize(v)
    @v = v
  end
end
EOF
	cat >"$OUT/sql/file_$i.sql" <<EOF
SELECT  id,name,  created_at  FROM  users  WHERE  id  =  $i  ORDER  BY  name;
EOF
	cat >"$OUT/xml/file_$i.xml" <<EOF
<root><item id="$i"><name>Item $i</name><value>true</value></item></root>
EOF
	cat >"$OUT/php/file_$i.php" <<EOF
<?php
function hello_$i(\$name) {
\$x = array("a" => 1, "b" => 2);
return \$name . \$x["a"];
}
EOF
	cat >"$OUT/hcl/file_$i.hcl" <<EOF
resource "aws_instance" "web$i" {
ami = "ami-12345"
instance_type="t2.micro"
tags={Name="web$i"}
}
EOF
	cat >"$OUT/c/file_$i.c" <<EOF
#include <stdio.h>
int hello_$i(const char *name) {
printf("%s %d\\n", name, $i);
return 0;
}
EOF
done

# Already-formatted samples for the unchanged path
for i in $(seq 1 10); do
	printf 'function ok() {\n\treturn 1;\n}\n' >"$OUT/js/ok_$i.js"
	printf 'def ok():\n\treturn 1\n' >"$OUT/py/ok_$i.py"
done

echo "Generated $(find "$OUT" -type f | wc -l | tr -d ' ') files under $OUT"

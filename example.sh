#!/bin/bash

bin="./target/release/csv2seeds2cbors"

input1(){
  echo id,subid,date8
  echo 42,333,20260717
  echo 42,334,20260718
  echo 43,634,20260718
  echo 43,635,20260719
}

input1 |
  ENV_WAT_PATH="./example.wat" "${bin}" |
  python3 -m cbor2.tool --pretty --sequence

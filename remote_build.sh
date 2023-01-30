#!/bin/bash
branch=''

while getopts "b:" arg; do
  case $arg in
    b) branch=$OPTARG;;
  esac
done

VAR=$(sed '2!d' < "master.conf")

# Split variable by comma
IFS="|" read -ra SPLITVAR <<< "$VAR"

trimmed_user="${SPLITVAR[0]//[[:blank:]]/}"
trimmed_key="${SPLITVAR[1]//[[:blank:]]/}"

echo $trimmed_user
echo $trimmed_key

input="./nodes.conf"
while IFS= read -r line
do
  IFS='|' read -r -a splitted <<< "$line"
  # echo "$line"
  #splitted=$(echo $line | tr "|" "\n")

  #echo $splitted
  echo ${splitted[0]}
  ip=${splitted[0]}
  dir=${splitted[1]}

  if [ -z "$branch" ]
  then
        ssh $trimmed_user@$ip -i $trimmed_key "cd $dir; ./build.sc"
  else
        ssh $trimmed_user@$ip -i $trimmed_key "cd $dir; git checkout $branch; ./build.sc"
  fi  
done < "$input"
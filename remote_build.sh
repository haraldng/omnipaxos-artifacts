#!/bin/bash
branch=''

while getopts "b:" arg; do
  case $arg in
    b) branch=$OPTARG;;
  esac
done


input="./nodes.conf"
while IFS= read -r line
do
  IFS='|' read -r -a splitted <<< "$line"
  # echo "$line"
  #splitted=$(echo $line | tr "|" "\n")

  #echo $splitted
  echo ${splitted[0]}
  ip=${splitted[0]}
  dir= ${splitted[1]}

  if [ -z "$branch" ]
  then
        ssh $1@$ip "cd $dir; ./build.sc"
  else
        ssh $1@$ip "cd $dir; git checkout $branch; ./build.sc"
  fi  
done < "$input"
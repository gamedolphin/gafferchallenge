#!/bin/bash

for i in {0..5}
do
    nohup /client --server-addr 10.0.2.1:3001 --frequency 100 &> $i.out &
done



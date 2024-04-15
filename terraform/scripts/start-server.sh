#!/bin/bash

for i in {0..5}
do
    nohup /server --port 3001 --backend-addr 10.0.3.1:3000 &> $i.out &
done

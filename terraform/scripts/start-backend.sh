#!/bin/bash

for i in {0..5}
do
    nohup /backend --port 3000 &> $i.out &
done

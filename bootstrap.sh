#!/bin/bash

mise trust
cp ../telegram-llm-tui/.env .
mkdir data
cp ../telegram-llm-tui/data/cache.sqlite ./data/
cp ../telegram-llm-tui/data/telegram.session ./data/
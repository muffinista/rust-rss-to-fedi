#!/bin/bash

rsync -av --delete --exclude target --exclude profile --exclude db --exclude data --exclude .env . colin@$DEST:$DESTDIR


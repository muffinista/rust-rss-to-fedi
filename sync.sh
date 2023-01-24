#!/bin/bash

rsync -av --delete --exclude target --exclude profile --exclude data . colin@$DEST:$DESTDIR


#!/bin/bash

rsync -av --delete --exclude target --exclude database.sqlite . colin@$DEST:$DESTDIR


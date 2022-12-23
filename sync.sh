#!/bin/bash

rsync -av --delete --exclude target --exclude data . colin@$DEST:$DESTDIR


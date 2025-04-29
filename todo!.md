Solution for the problem: add commands insert/copy chunk and make it so InsertChunk gets a lot of virtual space so there is no problem with fitting it in

Add new command type: insert packed/chunk | consider adding command CopyDirrectly
Make get_changes to get changes in the files, and then use it as an input for recover
Make compare, that returns weather file changed

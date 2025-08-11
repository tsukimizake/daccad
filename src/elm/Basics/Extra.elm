module Basics.Extra exposing (noCmd, withCmd, withCmds)


noCmd : model -> ( model, Cmd msg )
noCmd m =
    ( m, Cmd.none )


withCmd : Cmd msg -> model -> ( model, Cmd msg )
withCmd cmd m =
    ( m, cmd )


withCmds : List (Cmd msg) -> model -> ( model, Cmd msg )
withCmds cmds m =
    ( m, Cmd.batch cmds )

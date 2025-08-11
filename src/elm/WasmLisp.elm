port module WasmLisp exposing (fromWasm, getStlBytes, toWasm)

import Generated exposing (FromElmMessage(..), ModelId(..), ToElmMessage(..), fromElmMessageEncoder, toElmMessageDecoder)
import Json.Decode as Decode exposing (decodeValue)
import Json.Encode as Encode
import StlDecoder exposing (Stl)



-- PORTS


port fromElm : Encode.Value -> Cmd msg


port toElm : (Decode.Value -> msg) -> Sub msg



-- Internal message type to handle filtering
-- PUBLIC API


toWasm : FromElmMessage -> Cmd msg
toWasm message =
    fromElm (fromElmMessageEncoder message)


getStlBytes : ModelId -> Cmd msg
getStlBytes (ModelId id) =
    toWasm (GetStlBytes { modelId = id })


fromWasm : (Maybe ToElmMessage -> msg) -> Sub msg
fromWasm tagger =
    Sub.map tagger <|
        toElm <|
            \value ->
                case decodeValue toElmMessageDecoder value of
                    Ok msg ->
                        Just msg

                    Err _ ->
                        Nothing

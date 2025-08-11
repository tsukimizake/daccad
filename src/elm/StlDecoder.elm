module StlDecoder exposing (Stl, Vec, run)

import Bytes exposing (Bytes, Endianness(..))
import Bytes.Decode as BD
import Bytes.Encode as BE



--
-- STL FILE FORMAT
-- UINT8[80]    – Header                 -     80 bytes
-- UINT32       – Number of triangles    -      4 bytes
-- foreach triangle                      - 50 bytes:
--     REAL32[3] – Normal vector             - 12 bytes
--     REAL32[3] – Vertex 1                  - 12 bytes
--     REAL32[3] – Vertex 2                  - 12 bytes
--     REAL32[3] – Vertex 3                  - 12 bytes
--     UINT16    – Attribute byte count      -  2 bytes
-- end
--


type alias Vec =
    ( Float, Float, Float )


type alias Stl =
    { header : String
    , numTriangles : Int
    , triangles : List ( Vec, Vec, Vec )
    }


run : List Int -> Maybe Stl
run stlBytes =
    stlBytes
        |> encodeToBytes
        |> decodeStl


encodeToBytes : List Int -> Bytes
encodeToBytes stlBytes =
    stlBytes
        |> List.map BE.unsignedInt8
        |> BE.sequence
        |> BE.encode


decodeStl : Bytes -> Maybe Stl
decodeStl bytes =
    let
        stlDecoder : BD.Decoder Stl
        stlDecoder =
            BD.map2 (\a ( b, c ) -> Stl a b c)
                (BD.string 80)
                (BD.unsignedInt32 LE
                    |> BD.andThen
                        (\len ->
                            BD.loop ( len, [] )
                                (\( l, xs ) ->
                                    if l == 0 then
                                        BD.succeed <| BD.Done ( len, xs )

                                    else
                                        BD.map (\x -> BD.Loop ( l - 1, x :: xs )) triangleDecoder
                                )
                        )
                )
    in
    BD.decode stlDecoder bytes


triangleDecoder : BD.Decoder ( Vec, Vec, Vec )
triangleDecoder =
    let
        vecDecoder =
            BD.map3 (\a b c -> ( a, b, c )) (BD.float32 LE) (BD.float32 LE) (BD.float32 LE)
    in
    BD.map4 (\_ b c d -> ( b, c, d )) vecDecoder vecDecoder vecDecoder vecDecoder
        |> BD.andThen (\r -> BD.bytes 2 |> BD.map (\_ -> r))

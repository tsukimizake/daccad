module Main exposing (main)

import Angle
import Basics.Extra exposing (noCmd, withCmd, withCmds)
import Browser
import Color
import Css
import Css.Extra exposing (..)
import Generated exposing (FromElmMessage(..), ModelId(..), ToElmMessage(..), ValueInner)
import Html.Styled exposing (..)
import Html.Styled.Attributes exposing (css, value)
import Html.Styled.Events exposing (..)
import Input exposing (textInput)
import Point3d
import Scene
import Scene3d
import Scene3d.Material as Material
import StlDecoder exposing (Stl)
import Triangle3d
import WasmBridge



-- MAIN


main : Program () Model Msg
main =
    Browser.element
        { init = init
        , update = update
        , subscriptions = subscriptions
        , view = Html.Styled.toUnstyled << view
        }



-- MODEL


type alias Model =
    { sourceFilePath : String
    , sourceCode : String
    , console : List String
    , previews : List PreviewConfig
    , wasmInitialized : Bool
    }


type alias PreviewConfig =
    { stlId : Int
    , stl : Stl
    , isDragging : Bool
    , sceneModel : Scene.Model
    }


init : () -> ( Model, Cmd Msg )
init _ =
    { sourceFilePath = "../hoge.pl"
    , sourceCode = ""
    , console = []
    , previews = []
    , wasmInitialized = False
    }
        |> noCmd


createPreviewConfig : Int -> Stl -> PreviewConfig
createPreviewConfig id stl =
    let
        viewPoint =
            ( 100, 100, 100 )

        ( x, y, z ) =
            viewPoint

        distance =
            sqrt (x * x + y * y + z * z)

        azimuth =
            Angle.radians (atan2 y x)

        elevation =
            Angle.radians (asin (z / distance))
    in
    { stlId = id
    , stl = stl
    , isDragging = False
    , sceneModel =
        { rotatexy = azimuth
        , elevation = elevation
        , distance = distance
        , viewPoint = viewPoint
        }
    }



-- UPDATE


type Msg
    = FromWasm ToElmMessage
    | ToWasm FromElmMessage
    | SetSourceFilePath String
    | SceneMsg Int Scene.Msg
    | ShowSaveDialog Int
    | Nop


update : Msg -> Model -> ( Model, Cmd Msg )
update msg mPrev =
    case msg of
        FromWasm toElmMessage ->
            case toElmMessage of
                EvaluationResult { result } ->
                    case result of
                        Ok res ->
                            let
                                -- Request STL bytes for each model
                                requestStlBytesCommands =
                                    res.previewList
                                        |> List.map (\(ModelId id) -> WasmBridge.toWasm <| GetStlBytes { modelId = id })

                                successMsg =
                                    "Evaluation successful: " ++ valueToString res.value
                            in
                            { mPrev
                                | console = successMsg :: mPrev.console
                            }
                                |> withCmds requestStlBytesCommands

                        Err err ->
                            { mPrev | console = err :: mPrev.console }
                                |> noCmd

                StlBytes { modelId, bytes } ->
                    case StlDecoder.run bytes of
                        Just stl ->
                            let
                                newPreviewConfig =
                                    createPreviewConfig modelId stl

                                updatedPreviews =
                                    newPreviewConfig :: mPrev.previews

                                consoleMsg =
                                    "Added preview for model " ++ String.fromInt modelId
                            in
                            { mPrev
                                | previews = updatedPreviews
                                , console = consoleMsg :: mPrev.console
                            }
                                |> noCmd

                        Nothing ->
                            { mPrev
                                | console = ("Failed to decode STL for model " ++ String.fromInt modelId) :: mPrev.console
                            }
                                |> noCmd

                FileLoaded { path, content } ->
                    { mPrev
                        | sourceCode = content
                        , sourceFilePath = path
                        , console = ("Loaded file: " ++ path) :: mPrev.console
                    }
                        |> noCmd

                FileLoadError { error } ->
                    { mPrev
                        | console = ("File load error: " ++ error) :: mPrev.console
                    }
                        |> noCmd

                Error { message } ->
                    { mPrev
                        | console = ("Error: " ++ message) :: mPrev.console
                    }
                        |> noCmd

        ToWasm fromElmMessage ->
            mPrev |> withCmd (WasmBridge.toWasm <| fromElmMessage)

        SetSourceFilePath path ->
            { mPrev | sourceFilePath = path }
                |> noCmd

        SceneMsg previewId sceneMsg ->
            let
                updatedPreviews =
                    List.map
                        (\preview ->
                            if preview.stlId == previewId then
                                let
                                    ( updatedSceneModel, isDragging ) =
                                        Scene.update sceneMsg preview.sceneModel
                                in
                                { preview
                                    | isDragging = isDragging
                                    , sceneModel = updatedSceneModel
                                }

                            else
                                preview
                        )
                        mPrev.previews
            in
            { mPrev | previews = updatedPreviews }
                |> noCmd

        ShowSaveDialog previewId ->
            { mPrev | console = ("Save dialog for preview " ++ String.fromInt previewId) :: mPrev.console }
                |> noCmd

        Nop ->
            mPrev |> noCmd



-- Helper function to convert ValueInner to string for display


valueToString : ValueInner -> String
valueToString value =
    case value of
        Generated.Integer n ->
            String.fromInt n

        Generated.Double f ->
            String.fromFloat f

        Generated.String s ->
            "\"" ++ s ++ "\""

        Generated.Symbol s ->
            s

        Generated.Stl (Generated.ModelId id) ->
            "<stl:" ++ String.fromInt id ++ ">"

        Generated.List vals ->
            "(" ++ String.join " " (List.map valueToString vals) ++ ")"



-- SUBSCRIPTIONS


subscriptions : Model -> Sub Msg
subscriptions model =
    let
        draggingSubs =
            model.previews
                |> List.filter .isDragging
                |> List.map
                    (\preview ->
                        Sub.map (SceneMsg preview.stlId) (Scene.subscriptions True)
                    )

        nonDraggingSubs =
            if List.any .isDragging model.previews then
                []

            else
                model.previews
                    |> List.map
                        (\preview ->
                            Sub.map (SceneMsg preview.stlId) (Scene.subscriptions False)
                        )
    in
    Sub.batch <|
        WasmBridge.fromWasm
            (\mmsg ->
                case mmsg of
                    Just msg ->
                        FromWasm msg

                    Nothing ->
                        Nop
            )
            :: draggingSubs
            ++ nonDraggingSubs



-- VIEW


view : Model -> Html Msg
view model =
    div
        [ css
            [ Css.displayFlex
            , Css.height (Css.vh 100)
            ]
        ]
        [ -- Left panel for code editor and console
          div
            [ css
                [ Css.width (Css.pct 30)
                , Css.displayFlex
                , Css.flexDirection Css.column
                , Css.borderRight3 (Css.px 1) Css.solid (Css.rgb 200 200 200)
                ]
            ]
            [ -- File path input
              div
                [ css [ Css.padding (Css.px 10) ] ]
                [ div [ css [ Css.width (Css.pct 100) ] ]
                    [ textInput model.sourceFilePath SetSourceFilePath ]
                ]

            -- Load and evaluate buttons
            , div
                [ css [ Css.padding (Css.px 10), Css.displayFlex, Css.property "gap" "10px" ] ]
                [ button
                    [ onClick (ToWasm <| LoadFile { filePath = model.sourceFilePath })
                    , css
                        [ Css.padding2 (Css.px 8) (Css.px 16)
                        , Css.backgroundColor (Css.rgb 70 130 180)
                        , Css.color (Css.rgb 255 255 255)
                        , Css.border Css.zero
                        , Css.borderRadius (Css.px 4)
                        , Css.cursor Css.pointer
                        ]
                    ]
                    [ text "Load" ]
                , button
                    [ onClick (ToWasm <| EvalCode { code = model.sourceCode })
                    , css
                        [ Css.padding2 (Css.px 8) (Css.px 16)
                        , Css.backgroundColor (Css.rgb 34 139 34)
                        , Css.color (Css.rgb 255 255 255)
                        , Css.border Css.zero
                        , Css.borderRadius (Css.px 4)
                        , Css.cursor Css.pointer
                        ]
                    ]
                    [ text "Evaluate" ]
                ]

            -- Code editor
            , div
                [ css
                    [ Css.flexGrow (Css.int 1)
                    , Css.padding (Css.px 10)
                    ]
                ]
                [ textarea
                    [ css
                        [ Css.width (Css.pct 100)
                        , Css.height (Css.pct 60)
                        , Css.fontFamily Css.monospace
                        , Css.fontSize (Css.px 14)
                        , Css.border3 (Css.px 1) Css.solid (Css.rgb 200 200 200)
                        , Css.borderRadius (Css.px 4)
                        , Css.padding (Css.px 8)
                        , Css.resize Css.none
                        ]
                    , onInput SetSourceFilePath
                    , value model.sourceCode
                    ]
                    []

                -- Console
                , div
                    [ css
                        [ Css.height (Css.pct 40)
                        , Css.marginTop (Css.px 10)
                        , Css.border3 (Css.px 1) Css.solid (Css.rgb 200 200 200)
                        , Css.borderRadius (Css.px 4)
                        , Css.padding (Css.px 8)
                        , Css.backgroundColor (Css.rgb 248 248 248)
                        , Css.overflowY Css.auto
                        , Css.fontFamily Css.monospace
                        , Css.fontSize (Css.px 12)
                        ]
                    ]
                    (model.console
                        |> List.reverse
                        |> List.map (\msg -> div [] [ text msg ])
                    )
                ]
            ]

        -- Right panel for 3D previews
        , div
            [ css
                [ Css.width (Css.pct 70)
                , Css.displayFlex
                , Css.flexDirection Css.column
                , Css.padding (Css.px 10)
                , Css.property "gap" "10px"
                , Css.overflowY Css.auto
                ]
            ]
            (if List.isEmpty model.previews then
                [ div
                    [ css
                        [ Css.textAlign Css.center
                        , Css.marginTop (Css.px 50)
                        , Css.color (Css.rgb 128 128 128)
                        ]
                    ]
                    [ text "No previews to display. Evaluate some code to see results." ]
                ]

             else
                List.map viewPreview model.previews
            )
        ]


viewPreview : PreviewConfig -> Html Msg
viewPreview config =
    div
        [ css
            [ Css.border3 (Css.px 1) Css.solid (Css.rgb 200 200 200)
            , Css.borderRadius (Css.px 8)
            , Css.padding (Css.px 15)
            , Css.backgroundColor (Css.rgb 255 255 255)
            ]
        ]
        [ -- Preview header
          div
            [ css
                [ Css.displayFlex
                , Css.justifyContent Css.spaceBetween
                , Css.alignItems Css.center
                , Css.marginBottom (Css.px 10)
                ]
            ]
            [ h3
                [ css [ Css.margin Css.zero, Css.fontSize (Css.px 16) ] ]
                [ text ("Preview " ++ String.fromInt config.stlId) ]
            , button
                [ onClick (ShowSaveDialog config.stlId)
                , css
                    [ Css.padding2 (Css.px 6) (Css.px 12)
                    , Css.backgroundColor (Css.rgb 220 220 220)
                    , Css.border Css.zero
                    , Css.borderRadius (Css.px 4)
                    , Css.cursor Css.pointer
                    , Css.fontSize (Css.px 12)
                    ]
                ]
                [ text "Save STL" ]
            ]

        -- 3D Scene
        , div
            [ css
                [ Css.height (Css.px 300)
                , Css.border3 (Css.px 1) Css.solid (Css.rgb 230 230 230)
                , Css.borderRadius (Css.px 4)
                , Css.overflow Css.hidden
                ]
            ]
            [ Html.Styled.map (SceneMsg config.stlId) <|
                Scene.preview config.sceneModel triangleToEntity config.stl
            ]
        ]


triangleToEntity : ( ( Float, Float, Float ), ( Float, Float, Float ), ( Float, Float, Float ) ) -> Scene3d.Entity coordinates
triangleToEntity ( ( x1, y1, z1 ), ( x2, y2, z2 ), ( x3, y3, z3 ) ) =
    Scene3d.triangle (Material.color Color.lightBlue) <|
        Triangle3d.from
            (Point3d.meters x1 y1 z1)
            (Point3d.meters x2 y2 z2)
            (Point3d.meters x3 y3 z3)

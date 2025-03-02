module Scene exposing (Model, Msg(..), preview, update, subscriptions)

import Angle exposing (Angle)
import Browser.Events
import Camera3d exposing (Camera3d)
import Color
import Direction3d
import Html.Styled exposing (..)
import Json.Decode as Decode exposing (Decoder)
import Length exposing (Meters)
import Pixels exposing (Pixels, int)
import Point3d
import Quantity exposing (Quantity)
import Scene3d exposing (backgroundColor)
import SketchPlane3d
import StlDecoder exposing (Vec)
import Viewpoint3d


type alias Model =
    { azimuth : Angle
    , elevation : Angle
    , distance : Float
    , orbiting : Bool
    , viewPoint : Vec
    }


type Msg
    = MouseDown
    | MouseUp
    | MouseMove (Quantity Float Pixels) (Quantity Float Pixels)
    | MouseWheel Float


update : Msg -> Model -> Model
update message model =
    case message of
        -- Start orbiting when a mouse button is pressed
        MouseDown ->
            { model | orbiting = True }

        -- Stop orbiting when a mouse button is released
        MouseUp ->
            { model | orbiting = False }

        -- Orbit camera on mouse move (if a mouse button is down)
        MouseMove dx dy ->
            if model.orbiting then
                let
                    -- How fast we want to orbit the camera (orbiting the
                    -- camera by 1 degree per pixel of drag is a decent default
                    -- to start with)
                    rotationRate =
                        Angle.degrees 1 |> Quantity.per Pixels.pixel

                    -- Adjust azimuth based on horizontal mouse motion
                    newAzimuth =
                        model.azimuth
                            |> Quantity.minus (dx |> Quantity.at rotationRate)

                    -- Adjust elevation based on vertical mouse motion
                    -- and clamp to avoid camera flipping over
                    newElevation =
                        model.elevation
                            |> Quantity.plus (dy |> Quantity.at rotationRate)
                            |> Quantity.clamp (Angle.degrees -90) (Angle.degrees 90)
                in
                { model | azimuth = newAzimuth, elevation = newElevation }

            else
                model


-- Decoder for mouse movement
decodeMouseMove : Decoder Msg
decodeMouseMove =
    Decode.map2 MouseMove
        (Decode.field "movementX" (Decode.map Pixels.float Decode.float))
        (Decode.field "movementY" (Decode.map Pixels.float Decode.float))


subscriptions : Model -> Sub Msg
subscriptions model =
    if model.orbiting then
        -- If we're currently orbiting, listen for mouse moves and mouse button up events
        Sub.batch
            [ Browser.Events.onMouseMove decodeMouseMove
            , Browser.Events.onMouseUp (Decode.succeed MouseUp)
            ]
    else
        -- If we're not currently orbiting, just listen for mouse down events
        Browser.Events.onMouseDown (Decode.succeed MouseDown)


preview : Model -> (c -> Scene3d.Entity coordinates) -> { d | triangles : List c } -> Html Msg
preview model entity stl =
    Scene3d.sunny
        { upDirection = Direction3d.z
        , sunlightDirection = Direction3d.z
        , shadows = True
        , dimensions = ( int 400, int 400 )
        , camera = orbitingCamera model
        , clipDepth = Length.meters 1
        , background = backgroundColor Color.black
        , entities =
            List.map entity stl.triangles
        }
        |> Html.Styled.fromUnstyled


orbitingCamera : Model -> Camera3d Meters coordinates
orbitingCamera model =
    Camera3d.perspective
        { viewpoint =
            Viewpoint3d.orbit
                { focalPoint = Point3d.origin
                , groundPlane = SketchPlane3d.xy
                , azimuth = model.azimuth
                , elevation = model.elevation
                , distance = Length.meters model.distance
                }
        , verticalFieldOfView = Angle.degrees 30
        }

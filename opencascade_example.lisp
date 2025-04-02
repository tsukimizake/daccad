;; OpenCascade Example for daccad
;; This file demonstrates the new OpenCascade-based primitives

;; Define some variables for dimensions
(define box-width 60)
(define box-height 40)
(define box-depth 30)
(define cylinder-radius 15)
(define cylinder-height 50)

;; Create a main box
(define main-box (oc-box 0 0 0 box-width box-height box-depth))

;; Create a vertical hole
(define hole-position-x (/ box-width 2))
(define hole-position-y (/ box-height 2))
(define cylinder 
  (oc-cylinder hole-position-x hole-position-y 0 0 0 1 cylinder-radius cylinder-height))

;; Cut the cylinder from the box
(define part-with-hole (oc-cut main-box cylinder))

;; Create a smaller box for the second operation
(define small-box 
  (oc-box 
    (- box-width 20) 
    (- box-height 20) 
    (- box-depth 10) 
    20 20 20))

;; Combine with the holed part
(define final-part (oc-fuse part-with-hole small-box))

;; Chamfer the edges (when implemented)
;; (define chamfered-part (oc-chamfer final-part 2))

;; Preview the final shape
(preview final-part)

;; Alternative approach with threading macro
;; (-> (oc-box 0 0 0 box-width box-height box-depth)
;;     (oc-cut (oc-cylinder (/ box-width 2) (/ box-height 2) 0 0 0 1 cylinder-radius cylinder-height))
;;     (oc-fuse (oc-box (- box-width 20) (- box-height 20) (- box-depth 10) 20 20 20))
;;     (preview))
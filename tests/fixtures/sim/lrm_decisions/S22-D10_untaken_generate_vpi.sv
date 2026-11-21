// Decision S22-D10: nothing of a generate block that elaboration does not
// instantiate exists, so VPI shows none of it. An unnamed gate of an untaken
// block takes no number among the unnamed primitives of its module instance
// (decision A19-D6): the module's `or` is `or$1`. The untaken block, its
// variables, nets and named statement blocks are not found by name, while
// the selected block's are. A system function that only the untaken block
// calls is not called by the design, so an application may register its
// name (decision S99-D6): the registration of `$random` is accepted and
// never runs.
//
// IEEE 1800-2009 27.3 (SystemVerilog-1800-2009.txt L45986-45988): "The
// elaboration of a generate construct results in zero or more instances of
// a generate block. An instance of a generate block is similar in some ways
// to an instance of a module. It creates a new level of hierarchy. It brings
// the objects, behavioral constructs, and module instances within the block
// into existence." 27.5 L46364-46366: "The conditional generate constructs,
// if-generate and case-generate, select at most one generate block from a
// set of alternative generate blocks based on constant expressions evaluated
// during elaboration. The selected generate block, if any, is instantiated
// into the model."
//
// Build S22-D10_untaken_generate_vpi.c as a VPI application (llg: static
// archive given to --dpi-lib; `task $p()` declared as a system task).
`timescale 1ns/1ns
module tb;
  logic a = 1'b1, b = 1'b0;
  wire y0, y1, y3;
  integer r = 0;
  if (0) begin : u
    logic x = 1'b1;
    wire n;
    assign n = x;
    and (y0, a, b);
    initial begin : blk
      x = 1'b0;
      r = $random;
    end
  end
  case (1)
    0: begin : c0
      nand (y3, a, b);
    end
    default: begin : c1
    end
  endcase
  or (y1, a, b);
  if (1) begin : t
    logic z = 1'b1;
  end
  initial begin
    #1 $p;
    $display("hdl y1=%b r=%0d", y1, r);
    $finish;
  end
endmodule

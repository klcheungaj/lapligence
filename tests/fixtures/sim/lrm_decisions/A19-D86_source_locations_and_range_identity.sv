// Decision A19-D86: source locations of class properties, container
// elements, ranges and the range bounds and indices made per request, and
// the identity of ranges and those constants.
// - A property of a class definition or of a class object (a variable, an
//   event, an event array, a virtual interface) is on the line of its
//   declaration; an element of a dynamic or fixed-size array is on its
//   array's line (decision A19-D47).
// - A range is on the line of the declaration it belongs to. A range bound
//   (vpiLeftRange, vpiRightRange) and an index constant (vpiIndex) are
//   located at the scope that declares the object: the definition of a
//   module instance (the module header), a named block, a package; the
//   scope that declares the class for a class property.
// - Two requests for the same range give handles that compare equal; the
//   ranges of two declarations do not. Range bounds and indices made per
//   request are one object per value in their scope: equal values compare
//   equal, different values do not.
// A typespec, an enum constant and an object made at run time have no
// line, as in industry-standard behaviour.
//
// IEEE 1800-2009 37.3.3 (SystemVerilog-1800-2009.txt L57394-57396): "These
// properties are applicable to every object that corresponds to some object
// within the source code." The text does not say which line a range bound
// or an index made by the simulator reports. 38.3 (L61867-61868): "The VPI
// routine vpi_compare_objects() shall return 1 (TRUE) if the two handles
// refer to the same underlying simulation object at the time the function
// is called".
//
// Build A19-D86_source_locations_and_range_identity.c as a VPI application
// (llg: static archive given to --dpi-lib; `task $d86()` declared as a
// system task).
interface d86_if (input logic clk);
  logic [7:0] data;
endinterface

module d86_sub (input logic clk);
  logic [1:0] st = 2'd0;
  logic [3:0] smem [0:1];
  initial begin : blk
    logic [6:0] bv;
    bv = 7'd0;
    smem[0] = bv[3:0];
    st = smem[0][1:0];
  end
endmodule

module tb;
  class Packet;
    int id;
    event done;
    event steps [0:1];
    virtual d86_if vif;
    function new(int i);
      id = i;
    endfunction
  endclass
  logic clk = 1'b0;
  logic [3:0] a = 4'd1;
  logic [3:0] b = 4'd2;
  logic [3:0] mem [0:3];
  int da[];
  Packet p;
  d86_if bi (clk);
  d86_sub su (.clk(clk));
  initial begin
    p = new(7);
    p.vif = bi;
    da = new[2];
    mem[1] = a + b;
    #1 $d86;
    $display("done %0d %0d", p.id, mem[1]);
    $finish;
  end
endmodule

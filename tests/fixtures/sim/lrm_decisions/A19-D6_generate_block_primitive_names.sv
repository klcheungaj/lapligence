// Decision A19-D6 (unnamed primitives in generate blocks): a primitive
// instance written without a name inside a generate block is named in the
// scope of its module instance (`tb.xor$3`, not `tb.t.xor$3`), is iterated
// from the module, and is found only by that name. The `<n>` of
// `<keyword>$<n>` counts the unnamed primitives of the module instance
// from 1, UDP instances first, then gates; each kind is counted scope by
// scope: the module's own primitives in source order, then each generate
// block in source order, a block's own primitives before the blocks nested
// in it. A named primitive keeps its generate scope in its full name
// (`tb.t.g_named`).
//
// IEEE 1800-2009 37.31 (SystemVerilog-1800-2009.txt L59736-L59786) lists
// "-> name str: vpiName str: vpiFullName" without saying what an unnamed
// instance reports; 37.80 (L61661-L61731) gives a gen scope primitives
// without naming unnamed ones. The names and their scope are llg choices
// following industry-standard behaviour.
primitive u_mod (o, i);
  output o; input i;
  table 0 : 1; 1 : 0; endtable
endprimitive
primitive u_gen (o, i);
  output o; input i;
  table 0 : 0; 1 : 1; endtable
endprimitive
module tb;
  logic a = 1'b1, b = 1'b0;
  wire [9:0] y;
  if (1) begin : t
    u_gen (y[0], a);
    and (y[1], a, b);
    and g_named (y[2], a, b);
    if (1) begin : n
      or (y[3], a, b);
    end
    nand (y[4], a, b);
  end
  if (1) begin : s
    nor (y[5], a, b);
  end
  for (genvar i = 0; i < 2; i++) begin : l
    buf (y[6 + i], a);
  end
  xor (y[8], a, b);
  u_mod (y[9], b);
  initial begin
    #1 $d6g;
    $display("hdl y=%b", y);
    $finish;
  end
endmodule

// Decision A19-D38: UDP definitions as VPI objects. See docs/lrm_decisions.md
// for the decision and the expected output.
//
// IEEE 1800-2009 37.32 (SystemVerilog-1800-2009.txt L59798-L59825): a udp
// defn has vpiDefName, vpiSize (the number of inputs), vpiProtected,
// vpiPrimType, its io decls, its table entries (vpiSize: the number of
// symbol entries; a value) and an initial. Detail 1 (L59823): "Only string
// (decompilation) and vector (ASCII values) shall be obtained for table
// entry objects using vpi_get_value()". Detail 2 (L59825): "vpiPrimType
// returns vpiSeqPrim for sequential UDPs and vpiCombPrim for combinational
// UDPs." 38.15 (L62766-L62772): the vector holds ((size-1)/4+1)
// s_vpi_vecval structures, "Each symbol shall require two bytes", ordered
// "most significant byte of abit first, then the least significant byte of
// abit, then the most significant byte of bbit, and then the least
// significant byte of bbit"; a one-character symbol has "\0" as its second
// byte. The 38.15 example (L62902-L62922) shows the strings "10:?:1",
// "0(01):?:-" and "(10)0:0:1" for the rows "1 0 :?:1;", "0 (01) :?:-;" and
// "(10) 0 :0:1;".
//
// Text decides (another simulator returns the source text of the row with
// its blanks, edges as r/f, and packs those characters into the vector):
// the string is the inputs, then ":state" for a sequential UDP, then
// ":output", without blanks, an edge as "(vw)"; the vector holds four
// two-byte symbols per s_vpi_vecval, two in aval and two in bval, each the
// first character in the high byte of its 16-bit half.
primitive d38_and (out, a, b);
  output out;
  input a, b;
  table
    0 ? : 0;
    ? 0 : 0;
    1 1 : 1;
  endtable
endprimitive

primitive d38_ff (q, d, clk);
  output q;
  reg q;
  input d, clk;
  initial q = 1'b0;
  table
    ? (01) : ? : -;
    0 (10) : ? : 0;
    1 (10) : ? : 1;
  endtable
endprimitive

module tb;
  reg a = 0, b = 1, clk = 0;
  wire o, q;
  d38_and g1 (o, a, b);
  d38_ff f1 (q, a, clk);
  initial begin
    #1 $d38();
    $finish;
  end
endmodule

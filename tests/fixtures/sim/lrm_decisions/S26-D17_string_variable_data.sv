// Decision S26-D17 (llg choice): a data argument of $sreadmemb/$sreadmemh may
// be any string expression (a string variable, an array element, a
// concatenation) or an integral value holding text, not only a string literal. Each argument is read when the
// task runs, so a variable changed between two calls loads its new text;
// literals and expressions mix in one call and form one image as in S26-D16;
// an empty string contributes no words.
//
// IEEE 1800-2009 D.14 (SystemVerilog-1800-2009.txt L70157-70158): "The
// $sreadmemh and $sreadmemb system tasks take memory data values and
// addresses as string literal arguments." The syntax (L70152-70153) names the
// arguments `string`, and Annex D is informative (D.1, L69791: "for
// informative purposes only and are not part of this standard"), so the text
// does not forbid other string expressions; llg accepts them. IEEE 1364-2001
// C.13 (Verilog-1364-2001.txt L48054-48055) says "string arguments", which in
// Verilog-2001 includes text held in a reg vector.
module tb;
  logic [7:0] m [0:3];
  string image = "11 22";
  string tail;
  string empty = "";
  string parts [0:1];
  logic [8*5:1] packed_text;
  initial begin
    m = '{default: 8'h5a};
    $sreadmemh(m, 0, 1, image);
    $display("var %h %h %h %h", m[0], m[1], m[2], m[3]);
    image = "33 44 55";
    $sreadmemh(m, 0, 2, image);
    $display("changed %h %h %h %h", m[0], m[1], m[2], m[3]);
    tail = "bb";
    $sreadmemh(m, 0, 3, "aa", tail, "cc dd");
    $display("mixed %h %h %h %h", m[0], m[1], m[2], m[3]);
    m = '{default: 8'h5a};
    $sreadmemh(m, 0, 1, empty, "66", empty, "77");
    $display("empty %h %h %h %h", m[0], m[1], m[2], m[3]);
    parts[0] = "@2 1";
    parts[1] = "0";
    $sreadmemb(m, 0, 3, parts[0], parts[1]);
    $display("element %h %h %h %h", m[0], m[1], m[2], m[3]);
    $sreadmemh(m, 0, 1, {tail, " ", "ee"});
    $display("concat %h %h %h %h", m[0], m[1], m[2], m[3]);
    packed_text = "99 88";
    $sreadmemh(m, 2, 3, packed_text);
    $display("packed %h %h %h %h", m[0], m[1], m[2], m[3]);
    $finish;
  end
endmodule

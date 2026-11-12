// SIM-027 boundary: an element of a dynamic array, queue or associative array
// that is a fixed unpacked array of strings, handles or records is copied to
// or from an array variable only cell by cell; the whole-element copy rejects
// with its location. Rows of integral or real cells copy whole.
module tb;
  string sd[][2];
  string sr[2] = '{"x", "y"};
  initial begin
    sd = new[1]; sd[0] = sr;
    $display("sd %p", sd);
  end
endmodule

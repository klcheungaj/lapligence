// SIM-024: a format held in a variable is interpreted when the call runs
// (SV 21.3.3). A specification without an argument is printed as written
// and reported once on stderr; its arguments are not changed.
module tb;
  string f;
  int v;
  initial begin
    v = 7;
    f = "%0d|%0d";
    $display("A|%s|", $sformatf(f, v));
    $display("B|%s|", $sformatf(f, v));
    f = "%0d";
    $display("C|%s|%0d|", $sformatf(f, v), v);
    $finish(0);
  end
endmodule

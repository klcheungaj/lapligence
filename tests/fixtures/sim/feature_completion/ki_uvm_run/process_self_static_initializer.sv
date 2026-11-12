`timescale 1ns/1ns
// process::self() in static declaration initializers and in an initial
// procedure. Known issue: llg returns null in the initializers, where the
// industry-standard simulator returns a running process (see the readme).
module tb;
  function automatic int probe(string tag);
    process p = process::self();
    string s;
    int a, b;
    $display("%s: null=%0d", tag, p == null);
    if (p != null) begin
      $display("%s: status=%s", tag, p.status().name());
      s = p.get_randstate();
      a = $urandom;
      p.set_randstate(s);
      b = $urandom;
      $display("%s: replay=%0d", tag, a == b);
    end
    return 1;
  endfunction
  int x = probe("static");
  int y = probe("static2");
  initial begin
    int z;
    z = probe("initial");
    $finish;
  end
endmodule

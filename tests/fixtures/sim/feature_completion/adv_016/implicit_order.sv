// One rule generates eight value productions of different types, so it has
// eight implicit result variables (IEEE 1800-2009 18.17.7). Their order must
// not depend on where the frontend allocated the productions: generating the
// model twice gives the same C.
module tb;
  initial begin
    randsequence (main)
      main : a b c d e f g h
        { $display("%0d %0d %0d %b %0d %s %0d %0d", a, b, c, d, e, f, g, h); };
      int a : { return 1; };
      byte b : { return 2; };
      shortint c : { return 3; };
      logic [3:0] d : { return 4'b0101; };
      longint e : { return 5; };
      string f : { return "six"; };
      bit [6:0] g : { return 7; };
      integer h : { return 8; };
    endsequence
    $finish;
  end
endmodule

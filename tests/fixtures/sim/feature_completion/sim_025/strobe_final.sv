// SIM-025 A01: $strobe prints once per call at the end of its slot with the
// values settled after every Active, Inactive and NBA event of the slot
// (SV 4.4.2.9, 21.2.2). Reports of one slot appear in call order.
module tb;
  reg [7:0] a = 8'h01;
  reg [3:0] x = 4'b0000;
  real r = 1.0;
  string s = "a";
  int q[$];

  function [7:0] inc(input [7:0] v);
    inc = v + 8'd1;
  endfunction

  initial begin
    $strobe("S0 a=%0d x=%b r=%0.2f s=%s", a, x, r, s);
    #1;
    $strobe("S1 a=%0d inc=%0d s=%s", a, inc(a), {s, s});
    a = 8'd10;
    q.push_back(7);
    $strobe("S2 q=%p a=%0d", q, a);
    a = 8'd20;
    a = 8'd30;
    x = 4'b10xz;
    r = -0.5;
    s = "late";
    #0 a = 8'd40;
    #1;
    for (int i = 0; i < 3; i++) begin
      a = a + 1;
      $strobe("L a=%0d q=%0d", a, q.size());
      q.push_back(i);
    end
    #1 $strobe("T x=%b r=%0.2f s=%s", x, r, s);
    #1 $finish(0);
  end

  initial begin
    #1 a <= 8'd50;
    #1 begin end
  end
endmodule

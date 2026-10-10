// SIM-014: repeat counts of intra-assignment event controls (V2001 9.7.7,
// SV 9.4.5). The RHS is evaluated first, then the count once; a count that
// is X/Z, nonpositive (signed) or a real that rounds to zero makes the
// assignment occur at once, as if there were no repeat construct. Blocking
// forms are in the first process, nonblocking forms in the second, whose
// operands the third process changes after each issue.
`timescale 1ns / 1ns
module tb;
  event e;
  int n, m;
  logic [3:0] xv, yv;
  logic [7:0] a, b, c, d;
  real r, rq;

  // `e` occurs at 2, 4, 6, ...
  initial forever #2 ->e;

  initial begin
    #1 n = 2;
    b = 8'h11;
    fork
      begin
        a = repeat (n) @e b;
        $display("%0t blocking a=%h", $time, a);
      end
      #2 begin
        b = 8'h22;
        n = 9;
      end
    join
    #1 n = 0;
    b = 8'h33;
    a = repeat (n) @e b;
    $display("%0t blocking zero a=%h", $time, a);
    n = -1;
    b = 8'h44;
    a = repeat (n) @e b;
    $display("%0t blocking negative a=%h", $time, a);
    xv = 4'bx;
    a = repeat (xv) @e 8'h55;
    $display("%0t blocking x a=%h", $time, a);
    xv = 4'b1z00;
    a = repeat (xv) @e 8'h56;
    $display("%0t blocking z a=%h", $time, a);
    r = 0.4;
    a = repeat (r) @e 8'h57;
    $display("%0t blocking real 0.4 a=%h", $time, a);
    r = 1.5;
    a = repeat (r) @e 8'h58;
    $display("%0t blocking real 1.5 a=%h", $time, a);
  end

  // Nonblocking forms: this process only issues; the next one changes the
  // counts and the RHS operand after each issue.
  initial begin
    #11 c <= repeat (m) @e d;
    #2 $display("%0t nba pending c=%h", $time, c);
    #2 $display("%0t nba c=%h", $time, c);
    c <= repeat (m) @e 8'h88;
    $display("%0t nba zero issued c=%h", $time, c);
    #1 $display("%0t nba zero c=%h", $time, c);
    #1 c <= repeat (m) @e 8'h99;
    #1 $display("%0t nba negative c=%h", $time, c);
    c <= repeat (yv) @e 8'haa;
    #1 $display("%0t nba z c=%h", $time, c);
    c <= repeat (rq) @e 8'hbb;
    #4 $display("%0t nba real c=%h", $time, c);
    $finish;
  end

  initial begin
    yv = 4'bz;
    rq = 2.0;
    #10 m = 2;
    d = 8'h66;
    #2 d = 8'h77;
    m = 7;
    #2 m = 0;
    #2 m = -5;
  end
endmodule

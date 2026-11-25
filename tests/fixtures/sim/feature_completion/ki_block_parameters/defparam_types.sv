// The final value gives a parameter without a type and a range its type and
// range, also when a defparam assigns it (6.20.2, 23.10).
module kinds;
  parameter A = 3;
  parameter B = 3;
  parameter C = 3;
  parameter D = 1;
  parameter F = 8'h1;
  parameter G = 4'd3;
  parameter signed H = 1;
  parameter signed J = 1;
  parameter [7:0] K = 1;
  parameter int L = 1;
  parameter M = 1;
  parameter N = "ab";
  parameter O = 1.5;
  parameter P = 7;
  parameter Q = 2;
  parameter signed [5:0] R = 1;
  parameter U = 1;
  localparam LD = D;
  localparam LS = $bits(D);
  localparam LC = {D, 4'hf};
  wire [D:0] w;
  logic [$bits(D)-1:0] v;
  string s;
  initial begin
    $display("%0d %0d", $bits(A), A);
    $display("%0d %0d", $bits(B), B);
    $display("%0d %0d", $bits(C), C);
    $display("%0d %0d %h [%d] %0d", $bits(D), D, D, D, $bits(LD));
    $display("%0d %0d %0d %0d %h", LS, $bits(w), $bits(v), $bits({D}), LC);
    $display("%0d %h", $bits(F), F);
    $display("%0d %h", $bits(G), G);
    $display("%0d %0d %b", $bits(H), H, H < 0);
    $display("%0d %0d %b", $bits(J), J, J < 0);
    $display("%0d %0d", $bits(K), K);
    $display("%0d %0d", $bits(L), L);
    $display("%0d %0d %b", $bits(M), M, M < 0);
    s = N;
    $display("%0d %s %s %0d", $bits(N), N, s, s.len());
    $display("%0.2f %0.2f", O, P);
    $display("%0d %h %s", $bits(Q), Q, Q);
    $display("%0d %0d", $bits(R), R);
    $display("%0d %b", $bits(U), U);
  end
endmodule

// A default value wider than the final one, and the same value in two sizes.
module sizes;
  parameter A = 5;
  parameter W = 40'hff_0000_0001;
  parameter S = "hello";
  localparam LA = A;
  task show;
    $display("%m %0d %0d %0d %0d %0d %s", $bits(A), $bits(LA), $bits(W), W, $bits(S), S);
  endtask
endmodule

// A size that a defparam of an outer level and a generate condition read.
module leaf;
  parameter W = 4;
  parameter V = 0;
  parameter X = V + 1;
  localparam type T = logic [$bits(V)-1:0];
  logic [W-1:0] r;
  T t;
  if ($bits(V) == 5) begin : sized
    task show; $display("five"); endtask
  end else begin : sized
    task show; $display("%0d", $bits(V)); endtask
  end
  function int f(); return $bits(V); endfunction
  task show;
    $display("%m %0d %0d %0d %0d %0d %0d %h", $bits(r), $bits(V), $bits(X), $bits(t), f(),
             $bits(W), {V, 2'b11});
    sized.show;
  endtask
endmodule
module mid;
  parameter P = 2;
  leaf l1();
  leaf l2();
  defparam l1.V = P;
  defparam l2.V = 5'h1f, l2.W = 3'd6;
endmodule

// Parameters of a named block and a function (23.10.2).
module blocks;
  initial begin : blk
    parameter P = 1;
    parameter signed PS = 1;
    parameter [3:0] PR = 1;
    parameter PN = "ab";
    parameter PX = 2'b01;
    parameter PD = 4;
    logic [P:0] l;
    string t;
    t = PN;
    #20 $display("%0d %0d %0d %b %0d %0d %0d %0s", $bits(P), $bits(PS), PS, PS < 0, $bits(PR),
                 $bits(PN), $bits(l), t);
    $display("%h %0d %b %0d", {P, PX}, $bits(PX), PX, $bits(PD));
  end
  function automatic int f();
    parameter FP = 1;
    return $bits(FP);
  endfunction
  initial #30 $display("%0d", f());
endmodule

module tb;
  localparam X = 3;
  kinds k();
  defparam k.A = 5;
  defparam k.B = 40'd5;
  defparam k.C = -2;
  defparam k.D = 8'd7;
  defparam k.F = 16'h1234;
  defparam k.G = 300;
  defparam k.H = 4'hf;
  defparam k.J = 4'sb1000;
  defparam k.K = 12'hfff;
  defparam k.L = 64'h1_0000_0002;
  defparam k.M = 4'sb1001;
  defparam k.N = "abcd";
  defparam k.O = 3;
  defparam k.P = 3.75;
  defparam k.Q = "xyz";
  defparam k.R = 12'hfff;
  defparam k.U = 5'b1xz01;

  sizes a1();
  sizes a2();
  sizes a3();
  sizes #(.A(5)) b1();
  sizes #(.A(40'd5)) b2();
  sizes #(.A(X + 8'd1)) b3();
  defparam a1.A = 5;
  defparam a2.A = 40'd5;
  defparam a3.A = 3'sd1;
  defparam a1.W = 4'd9;
  defparam a2.W = -1;
  defparam a1.S = 16'h4142;
  defparam a2.S = "";
  defparam b3.A = 2'd1;
  defparam b3.S = {"ab", "cd"};

  mid o1();
  mid o2();
  defparam o1.P = 9'd5;

  for (genvar i = 0; i < 2; i = i + 1) begin : g
    leaf l();
    defparam l.V = 6'd9 + i;
  end

  blocks q();
  defparam q.blk.P = 5'd6;
  defparam q.blk.PS = 4'hf;
  defparam q.blk.PR = 8'hff;
  defparam q.blk.PN = "longer";
  defparam q.blk.PX = 7'b1xz01x1;
  defparam q.f.FP = 6'd1;

  initial begin
    #10 a1.show;
    a2.show;
    a3.show;
    b1.show;
    b2.show;
    b3.show;
    o1.l1.show;
    o1.l2.show;
    o2.l1.show;
    o2.l2.show;
    g[0].l.show;
    g[1].l.show;
    #30 $finish;
  end
endmodule

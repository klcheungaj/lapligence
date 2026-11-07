// Many specializations of one generic class, by type and by value. Each
// specialization is a class of its own (SV 8.25); the generated model numbers
// them the same way on every run.
module tb;
  typedef struct packed { logic [3:0] a; } s4_t;
  class P #(type T = int, int W = 1);
    T pool[int];
    function void add(int k, T v); pool[k] = v; endfunction
    function int num(); return pool.num() * W; endfunction
  endclass
  initial begin
    automatic P#(int, 1) p0 = new;
    automatic P#(string, 2) p1 = new;
    automatic P#(byte, 3) p2 = new;
    automatic P#(s4_t, 4) p3 = new;
    automatic P#(real, 5) p4 = new;
    automatic P#(int, 6) p5 = new;
    automatic P#(logic [7:0], 7) p6 = new;
    automatic P#(shortint, 8) p7 = new;
    p0.add(1, 5); p1.add(1, "x"); p2.add(1, 8'd3); p3.add(1, 4'h2);
    p4.add(1, 1.5); p5.add(1, 6); p5.add(2, 7); p6.add(1, 8'hff); p7.add(3, 9);
    $display("%0d %0d %0d %0d %0d %0d %0d %0d", p0.num(), p1.num(), p2.num(),
             p3.num(), p4.num(), p5.num(), p6.num(), p7.num());
    $finish;
  end
endmodule

// SV2009 6.21, 13.3.1, 13.4.2, 23.8, 25.7, 26.3: an explicitly static variable
// inside an automatic task, function or block is initialized once, before
// time 0, even when a declaration initializer calls its subroutine first.
// Automatic variables initialize on every entry. Static state belongs to the
// declaring module instance, interface instance, generate scope or package.
package cnt_pkg;
  int base = 50;
  function automatic int next_id();
    static int id = base;
    id++;
    return id;
  endfunction
endpackage

interface counter_if #(parameter int W = 1);
  int width_seen = W * 2;
  function automatic int stamp();
    static int s = width_seen + 1;
    s += W;
    return s;
  endfunction
endinterface

module leaf #(parameter int K = 1) (output int first_o, output int id_o);
  int seed = K * 10;
  function automatic int count();
    static int c = seed;
    automatic int step = 1;
    c += step;
    step++;
    return c;
  endfunction
  int first = count();
  int pkg_id = cnt_pkg::next_id();
  assign first_o = first;
  assign id_o = pkg_id;
endmodule

module tb;
  int svar1 = 1;
  int a_first, a_id, b_first, b_id;
  int seen [0:17];
  int n = 0;
  int r;
  leaf #(1) u1(a_first, a_id);
  leaf #(2) u2(b_first, b_id);
  counter_if #(3) i3();
  counter_if #(5) i5();
  for (genvar g = 0; g < 2; g++) begin : gen
    int gv = g + 7;
    function automatic int gf();
      static int gs = gv * 2;
      gs++;
      return gs;
    endfunction
    int gr = gf();
  end
  function automatic int bump();
    static int count = svar1 + 10;
    int temp = 100;
    count++;
    temp++;
    return count * 1000 + temp;
  endfunction
  int first_bump = bump();
  task automatic tick(output int value);
    static int calls = svar1 * 10;
    automatic int each = 5;
    calls++;
    each++;
    value = calls * 100 + each;
  endtask
  initial begin
    for (int i = 0; i < 3; i++) begin
      automatic int loop3 = 0;
      for (int j = 0; j < 3; j++) begin
        loop3++;
        seen[n] = loop3;
        n++;
      end
    end
    for (int i = 0; i < 3; i++) begin
      static int loop2 = svar1 - 1;
      for (int j = 0; j < 3; j++) begin
        loop2++;
        seen[n] = loop2;
        n++;
      end
    end
    for (int k = 0; k < 18; k++) begin
      if (k > 0) $write(" ");
      $write("%0d", seen[k]);
    end
    $display("");
    $display("bump %0d %0d %0d", first_bump, bump(), bump());
    tick(r);
    $write("tick %0d ", r);
    tick(r);
    $display("%0d", r);
    #1;
    $display("leaf %0d %0d %0d %0d", a_first, b_first, a_id + b_id, u1.count() + u2.count());
    $display("iface %0d %0d %0d", i3.stamp(), i5.stamp(), i3.stamp());
    $display("gen %0d %0d %0d %0d", gen[0].gr, gen[1].gr, gen[1].gf(), cnt_pkg::next_id());
    $finish(0);
  end
endmodule

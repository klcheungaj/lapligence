// Classes, modules and interfaces that name themselves with other parameter
// values until the values stop changing or a generate block ends the chain.
`timescale 1ns/1ns

// Ends at the specialization that names itself.
class countdown #(int N = 0);
  typedef countdown #(N > 0 ? N - 1 : 0) next;
  static int depth = N;
  int value = N;
  next link;
  static function int sum();
    return N == 0 ? 0 : N + next::sum();
  endfunction
  static function int visit();
    depth = depth + 1;
    return N == 0 ? depth : depth + next::visit();
  endfunction
  function int chain();
    if (N == 0) return value;
    link = new;
    return value + link.chain();
  endfunction
endclass

typedef class odd;
class even #(int N = 0);
  static function int steps();
    return N == 0 ? 0 : 1 + odd #(N > 0 ? N - 1 : 0)::steps();
  endfunction
endclass
class odd #(int N = 0);
  static function int steps();
    return N == 0 ? 0 : 1 + even #(N > 0 ? N - 1 : 0)::steps();
  endfunction
endclass

// Each level has two branches of the same values, which are one class.
class tree #(int N = 0);
  static function longint leaves();
    return N == 0 ? 1 : tree #(N > 0 ? N - 1 : 0)::leaves()
                        + tree #(N > 1 ? N - 2 : 0)::leaves();
  endfunction
endclass

class box #(type T = int);
  T item;
endclass

module ladder #(int N = 0) (output int total);
  if (N == 0) begin : last
    assign total = 1;
  end else begin : step
    int below;
    ladder #(N - 1) next (below);
    assign total = below + 1;
  end
endmodule

module tb;
  int steps;
  ladder #(60) top (steps);
  // Nests the specializations of one class in a parameter assignment.
  typedef box #(box #(box #(box #(box #(box #(box #(box #(box #(box #(
      box #(int))))))))))) nested;
  nested n;
  countdown #(100) c;
  initial begin
    #1;
    n = new;
    n.item = new;
    n.item.item = new;
    n.item.item.item = new;
    c = new;
    $display("sum=%0d chain=%0d", countdown #(100)::sum(), c.chain());
    $display("visit=%0d again=%0d", countdown #(3)::visit(), countdown #(3)::visit());
    $display("even=%0d odd=%0d", even #(50)::steps(), odd #(33)::steps());
    $display("leaves=%0d", tree #(20)::leaves());
    $display("ladder=%0d", steps);
    $display("nested=%0d", n.item.item.item.item == null);
    $finish(0);
  end
endmodule

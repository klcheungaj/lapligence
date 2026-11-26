// Decision EO-D2: `value inside {m1, m2, ...}` whose set has a member with an
// effect (a call, an assignment operator) compares member by member, in set
// order, as `value ==? m1 || value ==? m2 || ...`: the scan stops at the
// first match, and a range `[lo:hi]` is `value >= lo && value <= hi`, so its
// high bound is evaluated only when the value is not below the low one. Each
// comparison reads the value again, before its member; a value with an
// effect of its own is evaluated once, first. A member that calls a function
// returning a queue is evaluated before the value and the other members. A
// `case inside` compares its selector, evaluated once, the same way.
//
// IEEE 1800-2009 11.4.13 (SystemVerilog-1800-2009.txt L15367-15369): "The
//   members of the set are scanned until a match is found and the operation
//   returns 1'b1. ... The order of evaluation of the expressions and ranges
//   is nondeterministic."
// IEEE 1800-2009 12.5 (L17489): "The case_expression shall be evaluated
//   exactly once and before any of the case_item_expressions."
module tb;
  typedef int iq_t[$];
  int arr[4];
  int q[$];
  int k, x, n;
  real rv;
  string sv;
  int log_q[$];
  function automatic int bump();
    k = 1;
    n++;
    return 1;
  endfunction
  function automatic int setk10(int v);
    k = v;
    return 10;
  endfunction
  function automatic int g(int v);
    log_q.push_back(v);
    return v;
  endfunction
  function automatic iq_t gq();
    log_q.push_back(8);
    return '{11 + k};
  endfunction
  function automatic iq_t gk();
    k = 1;
    return '{30};
  endfunction
  function automatic int popq();
    void'(q.pop_front());
    return 2;
  endfunction
  function automatic real setr();
    rv = 2.5;
    return 7.0;
  endfunction
  function automatic string sets();
    sv = "b";
    return "z";
  endfunction
  task automatic reset();
    arr = '{10, 20, 30, 40};
    q = '{1, 2, 3};
    k = 0; x = 0; n = 0; rv = 1.5; sv = "a";
    log_q.delete();
  endtask
  initial begin
    reset(); x = arr[k] inside {bump(), 10};
    $display("A %0d", x);
    reset(); x = g(1) inside {g(2), g(3), [g(4):g(5)], g(6)};
    $display("B %0d %p", x, log_q);
    reset(); x = g(2) inside {g(2), g(3), g(4)};
    $display("C %0d %p", x, log_q);
    reset(); x = g(4) inside {[g(3):g(5)], g(6)};
    $display("D %0d %p", x, log_q);
    reset(); x = popq() inside {q};
    $display("E %0d %p", x, q);
    reset(); x = arr[k] inside {[bump():15]};
    $display("F %0d", x);
    reset(); x = g(1) + (g(2) inside {g(3), g(2)}) + g(4);
    $display("G %0d %p", x, log_q);
    reset(); x = (arr[k] inside {bump(), 10}) && (arr[k] == 20);
    $display("H %0d", x);
    reset(); x = g(9) inside {g(1), arr};
    $display("I %0d %p", x, log_q);
    reset(); x = (k++) inside {arr[k], 0};
    $display("J %0d k=%0d", x, k);
    reset(); x = arr[k] inside {(k = 1), 20};
    $display("K %0d", x);
    reset(); case (arr[k]) inside [bump():15]: x = 1; default: x = 2; endcase
    $display("L %0d", x);
    reset(); case (g(1)) inside g(2), [g(3):g(4)]: x = 1; g(1): x = 2; g(5): x = 3; endcase
    $display("M %0d %p", x, log_q);
    reset(); if (arr[k] inside {bump(), 10}) x = 1; else x = 2;
    $display("N %0d", x);
    reset(); x = arr[k] inside {setk10(1)};
    $display("O %0d", x);
    reset(); x = arr[k] inside {[setk10(1):10]};
    $display("P %0d", x);
    reset(); x = g(7) inside {[g(5):g(6)], [g(1):g(9)]};
    $display("Q %0d %p", x, log_q);
    reset(); x = rv inside {setr(), 2.5};
    $display("R %0d", x);
    reset(); x = sv inside {sets(), "b"};
    $display("S %0d", x);
    reset(); x = arr[k] inside {arr[k + 1], 10};
    $display("T %0d", x);
    reset(); x = g(1) inside {g(2), gq()};
    $display("U %0d %p", x, log_q);
    reset(); x = (k + 11) inside {bump(), gq()};
    $display("V %0d", x);
    reset(); x = arr[k] inside {10, gk()};
    $display("W %0d", x);
    $finish;
  end
endmodule

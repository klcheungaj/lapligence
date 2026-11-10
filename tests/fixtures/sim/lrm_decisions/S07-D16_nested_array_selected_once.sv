// Decision S07-D16: a queue or dynamic array nested in a container element
// that a statement names as a method receiver or as an output actual is
// selected once, when the statement starts, and stays that element until the
// statement ends. Two names of one element change the same array. A call in
// the statement that assigns the element whole changes the array the
// statement already selected, so a later push or copy-out lands in it. A
// call that deletes the element leaves the statement with an element that is
// no longer in the container: what the statement then writes is lost.
//
// IEEE 1800-2009 does not order the selection of a method's array against
// the evaluation of the method's arguments. 7.10.2.7
// (SystemVerilog-1800-2009.txt L9145): "The push_back() method inserts the
// given element at the end of the queue." 13.4 (L18754): "output
// // copy value out at end". 10.4.1 (L13025-13026), for assignments: "The
// order of evaluation of the variable_lvalue and the expression on the
// right-hand side is undefined if a timing control is not specified."
module tb;
  int qq[$][$];
  int a = 0, b = 0;
  int calls = 0;
  function automatic int f();
    calls++;
    return 0;
  endfunction
  function automatic int repl();
    qq[0] = {7, 8, 9};
    return 4;
  endfunction
  function automatic int drop();
    qq.delete(0);
    return 4;
  endfunction
  function automatic void fill(output int o[$]);
    qq[0] = {1};
    o = {5, 6};
  endfunction
  initial begin
    qq.push_back({1, 2, 3});
    qq.push_back({9});
    qq[a].push_back(qq[b].pop_front());
    $display("same element: %0d %0d %0d size %0d", qq[0][0], qq[0][1], qq[0][2], qq[0].size());
    qq[f()].push_back(7);
    $display("called selector: calls %0d size %0d last %0d", calls, qq[0].size(), qq[0][3]);
    qq[0].push_back(repl());
    $display("assigned by the argument: size %0d first %0d last %0d", qq[0].size(), qq[0][0], qq[0][3]);
    fill(qq[0]);
    $display("assigned by the callee: size %0d %0d %0d", qq[0].size(), qq[0][0], qq[0][1]);
    qq[0].push_back(drop());
    $display("deleted by the argument: outer %0d size %0d first %0d", qq.size(), qq[0].size(), qq[0][0]);
    $finish;
  end
endmodule

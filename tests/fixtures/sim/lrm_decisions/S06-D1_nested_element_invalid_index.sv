// Decision S06-D1: an element of a queue or dynamic array of containers
// selected with an index outside the current size is an invalid index. A
// method call or a write on it does nothing, and a read of it (its size, a
// pop, an element) returns the default of the element type; the run
// continues.
//
// IEEE 1800-2009 7.4.6 (SystemVerilog-1800-2009.txt L8235-8238): "If an index
// expression is out of the address bounds or if any bit in the address is X
// or Z, then the index shall be invalid. The result of reading from an array
// with an invalid index shall return the default uninitialized value for the
// array element type. Writing to an array with an invalid index shall perform
// no operation. Implementations may issue a warning if an invalid index
// occurs for a read or write operation of an array."
// 7.10 (L8979-8980): queues "can be manipulated using the indexing,
// concatenation, slicing operator syntax". Some simulators stop with a fatal
// error when such an element is used; llg follows the clause text.
module tb;
  int qq[$][$];
  int dq[][$];
  int qd[$][];
  int t[$];
  int n;
  initial begin
    t = '{3};
    qq.push_back(t);
    dq = new[1];
    qd.push_back('{4});
    qq[7].push_back(1);
    qq[7].push_front(2);
    qq[7].insert(0, 3);
    qq[7].delete();
    $display("methods: %p", qq);
    qq[7] = t;
    qq[7][0] = 5;
    qq[0][7] = 5;
    $display("writes: %p", qq);
    dq[5].push_back(1);
    qd[3][0] = 1;
    qd[0][3] = 1;
    $display("dynamic: %p %p", dq, qd);
    n = qq[7].size();
    $display("size: %0d", n);
    n = qq[7].pop_front();
    $display("pop: %0d %p", n, qq);
    n = qq[7][0];
    $display("element: %0d", n);
    qq[0].push_back(6);
    $display("after: %p", qq);
    $finish;
  end
endmodule

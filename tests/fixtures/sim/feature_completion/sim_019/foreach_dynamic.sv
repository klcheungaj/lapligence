// SIM-019: foreach over dynamic receivers (SV 12.7.3): queues, dynamic
// arrays, associative arrays in key order (integral and string keys),
// queues of queues (one and two loop variables) and records holding queues.
module tb;
  typedef struct {
    string name;
    int q[$];
  } bag_t;

  int q[$] = '{3, 1, 4, 1, 5};
  int d[] = '{10, 20};
  string sa[string];
  int ia[int];
  int qq[$][$];
  int row[$];
  bag_t bag;
  string text;
  int total;

  initial begin
    foreach (q[i]) total += q[i] * i;
    $display("queue %0d", total);
    foreach (d[k]) d[k] = d[k] + k;
    $display("dyn %0d %0d", d[0], d[1]);

    sa["pear"] = "p";
    sa["apple"] = "a";
    sa["fig"] = "f";
    text = "";
    foreach (sa[key]) text = {text, sa[key], key.substr(0, 0)};
    $display("string-keys %s", text);

    ia[5] = 50;
    ia[-5] = -50;
    ia[0] = 0;
    total = 0;
    foreach (ia[key]) total = total * 10 + (key + 6);
    $display("int-keys %0d", total);

    row = '{1, 2};
    qq.push_back(row);
    row = '{3, 4, 5};
    qq.push_back(row);
    total = 0;
    foreach (qq[i]) total += $size(qq[i]) * 10 + qq[i][0];
    $display("nested %0d", total);

    bag.name = "b";
    bag.q = '{7, 8, 9};
    total = 0;
    foreach (bag.q[i]) total += bag.q[i];
    $display("record-queue %0d", total);

    total = 0;
    foreach (qq[i, j]) total = total * 10 + qq[i][j] + i;
    $display("nested-2d %0d", total);
    $finish(0);
  end
endmodule

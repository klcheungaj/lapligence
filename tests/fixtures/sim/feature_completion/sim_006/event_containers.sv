// SIM-006 A02: events in queues, dynamic and associative arrays keep their
// trigger identity across reorder, insertion, reallocation and copy; null
// entries stay null and missing entries read as null (SV 6.17, 7.4.5, 7.5,
// 7.8.6, 7.10, 15.5). Event elements copy the handle, never the event.
`timescale 1ns/1ns
module tb;
  event q[$];
  event e1, e2, e3;
  event a[int];
  event s[string];
  event d[];
  event big[$];
  event got;

  function automatic int count_null(input event e[$]);
    int n = 0;
    foreach (e[i]) if (e[i] == null) n++;
    return n;
  endfunction

  task automatic fire_all(input event e[$]);
    foreach (e[i]) if (e[i] != null) ->e[i];
  endtask

  initial begin
    q.push_back(e1);
    q.push_back(e2);
    q.push_front(e3);
    q.insert(1, null);
    $display("size %0d null %0d nulls %0d", q.size(), q[1] == null, count_null(q));
    got = q[2];
    $display("alias %0d", got == e1);
    q.delete(1);
    $display("after delete %0d %0d", q.size(), q[1] == e1);
    got = q.pop_front();
    $display("pop %0d %0d", got == e3, q.size());
    a[5] = e2;
    s["k"] = e3;
    $display("missing %0d %0d exists %0d", a[9] == null, s["none"] == null, a.exists(5));
    d = new[2];
    $display("fresh %0d %0d", d[0] != null, d[0] != d[1]);
    d[1] = d[0];
    $display("shared %0d", d[0] == d[1]);
    fork
      begin @(d[1]); $display("d1 woke %0t", $time); end
      begin wait (q[0].triggered); $display("q0 triggered %0t", $time); end
      begin #1 ->d[0]; #1 ->q[0]; end
    join
    d = new[3](d);
    $display("resized shared %0d new %0d", d[0] == d[1], d[2] != d[0]);
    q.reverse();
    $display("reversed %0d %0d", q[0] == e2, q[1] == e1);
    for (int i = 0; i < 64; i++)
      if (i == 40) big.push_back(e2);
      else big.push_back(null);
    big.push_front(e1);
    $display("big %0d %0d %0d", big.size(), big[41] == e2, count_null(big));
    fork
      begin @(e1); $display("e1 via fire_all %0t", $time); end
      begin @(e2); $display("e2 via fire_all %0t", $time); end
      begin #1 fire_all(big); end
    join
    q = {};
    $display("empty %0d %0d", q.size(), q[0] == null);
    $finish(0);
  end
endmodule

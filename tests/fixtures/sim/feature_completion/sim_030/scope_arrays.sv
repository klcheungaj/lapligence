// SIM-030: std::randomize of arrays (IEEE 1800-2009 18.12, 18.5.7).
module tb;
  int d[];
  bit [3:0] q[$];
  bit [3:0] allowed[$];
  int bad, ok;

  function automatic int fill();
    byte local_d[];
    int r;
    r = std::randomize(local_d) with { local_d.size() == 4; foreach (local_d[i]) local_d[i] == 8'(i * 3); };
    return r + local_d[3];
  endfunction

  initial begin
    allowed = '{4'd1, 4'd9};
    for (int i = 0; i < 50; i++) begin
      ok = std::randomize(d, q) with {
        d.size() inside {[1:4]};
        q.size() == d.size();
        foreach (d[i]) d[i] == i * 10;
        foreach (q[j]) q[j] inside {allowed};
        q.sum() with (int'(item)) >= 1;
      };
      if (!ok || q.size() != d.size()) bad = bad + 1;
      for (int j = 0; j < d.size(); j++) if (d[j] != j * 10 || !(q[j] inside {1, 9})) bad = bad + 1;
    end
    $display("scope: bad=%0d", bad);
    $display("local: %0d", fill());
    $finish;
  end
endmodule

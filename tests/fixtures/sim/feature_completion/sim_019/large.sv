// SIM-019: larger receivers through the generated-loop and runtime paths:
// hashed unique over packed and string elements, keyed sorts of strings and
// records, and a captured-bound locator. The counts follow from the modular
// fill patterns.
module tb;
  typedef struct {
    string name;
    int rank;
  } rec_t;

  int q[$];
  int r[$];
  string s[$];
  string u[$];
  rec_t recs[$];
  int limit;

  initial begin
    for (int i = 0; i < 100000; i++) q.push_back((i * 7919) % 1000);
    r = q.unique();
    $display("int-unique %0d", r.size());
    limit = 990;
    r = q.find_index with (item >= limit);
    $display("int-capture %0d", r.size());

    for (int i = 0; i < 30000; i++) s.push_back($sformatf("k%0d", 10000 + (i * 17) % 3000));
    u = s.unique();
    $display("string-unique %0d", u.size());
    s.sort();
    $display("string-sort %s %s %s", s[0], s[10], s[29999]);
    s.rsort() with (item.substr(4, 5));
    $display("string-key %s", s[0].substr(4, 5));

    for (int i = 0; i < 20000; i++) recs.push_back('{$sformatf("r%0d", i), (i * 31) % 20000});
    recs.sort() with (item.rank);
    $display("record-sort %0d %0d %s", recs[0].rank, recs[19999].rank, recs[1].name);
    $finish(0);
  end
endmodule

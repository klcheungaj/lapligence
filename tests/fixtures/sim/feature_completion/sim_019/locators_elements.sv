// SIM-019: locator, min/max and unique methods over string, real, record
// and class-handle elements of queues, dynamic and associative arrays, for
// empty, single-element and larger receivers (SV 7.12.1). Result queues keep
// the element type; index results keep the receiver's index type.
module tb;
  typedef struct {
    string name;
    int age;
  } person_t;

  class Node;
    int v;
    string tag;
    function new(int value, string label);
      v = value;
      tag = label;
    endfunction
  endclass

  string s[$];
  string sr[$];
  string se[$];
  real r[];
  real rr[$];
  person_t p[$];
  person_t pr[$];
  Node n[$];
  Node nr[$];
  Node h;
  int idx[$];
  string sa[string];
  int ia[int];
  int iq[$];

  initial begin
    // Empty receivers produce empty results.
    sr = se.find with (item == "x");
    idx = se.find_index with (item.len() > 0);
    $display("empty %0d %0d", sr.size(), idx.size());
    sr = se.min();
    $display("empty-min %0d", sr.size());

    // Single element.
    s.push_back("solo");
    sr = s.max();
    idx = s.unique_index();
    $display("single %s %0d %0d", sr[0], idx.size(), idx[0]);

    s = '{"pear", "fig", "apple", "fig", "kiwi"};
    sr = s.find with (item.len() == 4);
    $display("find %0d %s %s", sr.size(), sr[0], sr[1]);
    idx = s.find_index with (item == "fig");
    $display("find_index %0d %0d %0d", idx.size(), idx[0], idx[1]);
    sr = s.find_first with (item > "b");
    idx = s.find_last_index with (item < "g");
    $display("first %s last %0d", sr[0], idx[0]);
    sr = s.min();
    $display("min %s", sr[0]);
    sr = s.max() with (item.len());
    $display("max-len %0d %s", sr.size(), sr[0]);
    sr = s.unique();
    $display("unique %0d %s %s %s %s", sr.size(), sr[0], sr[1], sr[2], sr[3]);

    r = new[5];
    r = '{2.5, -1.25, 7.0, -1.25, 0.5};
    rr = r.min();
    $display("rmin %0.2f", rr[0]);
    rr = r.max() with (item * item);
    $display("rmax-sq %0.2f", rr[0]);
    idx = r.unique_index();
    $display("runique %0d", idx.size());
    rr = r.find with (item < 1.0);
    $display("rfind %0d %0.2f %0.2f %0.2f", rr.size(), rr[0], rr[1], rr[2]);

    p.push_back('{"ann", 31});
    p.push_back('{"bob", 19});
    p.push_back('{"cid", 44});
    p.push_back('{"dee", 19});
    pr = p.find with (item.age < 30);
    $display("pfind %0d %s %s", pr.size(), pr[0].name, pr[1].name);
    pr = p.max with (item.age);
    $display("pmax %s", pr[0].name);
    pr = p.min with (item.name);
    $display("pmin %s", pr[0].name);
    idx = p.find_first_index with (item.name == "cid");
    $display("pfirst %0d", idx[0]);
    pr = p.unique with (item.age);
    $display("punique %0d", pr.size());

    for (int i = 0; i < 6; i++) begin
      h = new(i * 3 % 5, $sformatf("n%0d", i));
      n.push_back(h);
    end
    // v = 0, 3, 1, 4, 2, 0
    nr = n.find with (item.v > 2);
    $display("cfind %0d %s %s", nr.size(), nr[0].tag, nr[1].tag);
    nr = n.min with (item.v);
    $display("cmin %0d", nr.size());
    idx = n.find_index with (item == h);
    $display("chandle %0d %0d", idx.size(), idx[0]);
    nr = n.unique with (item.v);
    $display("cunique %0d", nr.size());

    // Associative receivers: index results are the keys themselves.
    sa["k3"] = "c";
    sa["k1"] = "a";
    sa["k2"] = "c";
    sr = sa.find with (item == "c");
    $display("sa-find %0d", sr.size());
    ia[-7] = 5;
    ia[12] = 5;
    ia[3] = 9;
    iq = ia.find_index with (item == 5);
    $display("ia-index %0d %0d %0d", iq.size(), iq[0], iq[1]);
    iq = ia.max();
    $display("ia-max %0d", iq[0]);

    // A larger receiver: 2000 elements with 37 distinct values.
    s.delete();
    for (int i = 0; i < 2000; i++) s.push_back($sformatf("v%0d", i % 37));
    sr = s.unique();
    idx = s.find_index with (item == "v5");
    $display("large %0d %0d %0d %0d", sr.size(), idx.size(), idx[0], idx[idx.size() - 1]);
    $finish(0);
  end
endmodule

// SIM-019: ordering by element type (SV 7.12.2), fixed-array shuffle, pops
// of non-packed elements (SV 7.10.2), dynamic array <-> queue assignment
// (SV 7.6), dimension queries of nested elements (SV 20.7) and copies of
// records holding queues (SV 7.2, 7.6). Shuffle results are checked only
// for what every permutation shares.
module tb;
  typedef struct {
    string name;
    int q[$];
  } bag_t;
  typedef struct {
    int id;
    bag_t inner;
  } holder_t;
  typedef struct {
    string name;
    int age;
  } person_t;

  class Item;
    int key;
    function new(int value);
      key = value;
    endfunction
  endclass

  string s[$] = '{"delta", "Alpha", "charlie", "bravo"};
  real r[] = '{0.5, -2.0, 3.25, 0.0};
  person_t p[$];
  person_t popped;
  Item items[$];
  Item h;
  int fa[8] = '{1, 2, 3, 4, 5, 6, 7, 8};
  byte fb[4:1] = '{8'd10, 8'd20, 8'd30, 8'd40};
  int dq[$];
  byte bq[$];
  int dd[];
  string ds[];
  string sq[$];
  int qq[$][$];
  int row[$];
  bag_t src;
  holder_t w;
  holder_t w2;
  int sum_before;
  int product;
  string word;

  initial begin
    s.sort();
    $display("sort %s %s %s %s", s[0], s[1], s[2], s[3]);
    s.rsort() with (item.len());
    $display("rsort-len %s %0d", s[0], s[3].len());
    r.rsort();
    $display("real %0.2f %0.2f %0.2f %0.2f", r[0], r[1], r[2], r[3]);
    r.sort() with (item * item);
    $display("real-key %0.2f %0.2f", r[0], r[3]);

    p.push_back('{"ann", 40});
    p.push_back('{"bob", 25});
    p.push_back('{"cat", 33});
    p.sort() with (item.age);
    $display("records %s %s %s", p[0].name, p[1].name, p[2].name);
    p.rsort() with (item.name);
    $display("records-name %s %s", p[0].name, p[2].name);
    word = p.pop_front().name;
    popped = p.pop_back();
    $display("pop %s %s %0d %0d", word, popped.name, popped.age, p.size());

    for (int i = 0; i < 5; i++) begin
      h = new((i * 7) % 5);
      items.push_back(h);
    end
    items.sort() with (item.key);
    $display("class %0d %0d %0d", items[0].key, items[2].key, items[4].key);
    h = items.pop_back();
    $display("class-pop %0d %0d", h.key, items.size());
    sq = '{"one", "two"};
    word = sq.pop_back();
    $display("string-pop %s %0d", word, sq.size());

    sum_before = fa.sum();
    fa.shuffle();
    product = 1;
    foreach (fa[i]) product *= fa[i];
    $display("shuffle %0d %0d %0d", sum_before, fa.sum(), product);
    fb.shuffle();
    bq = fb.max();
    $display("shuffle-desc %0d %0d", fb.sum() with (int'(item)), bq[0]);

    dd = '{3, 1, 2};
    dq = dd;
    dq.push_back(9);
    dd = dq;
    $display("dyn-queue %0d %0d %0d", dq.size(), dd.size(), dd[3]);
    ds = new[2];
    ds[0] = "x";
    ds[1] = "y";
    sq = ds;
    sq.push_front("w");
    ds = sq;
    $display("dyn-queue-string %0d %s %s", ds.size(), ds[0], ds[2]);

    row = '{5, 6, 7};
    qq.push_back(row);
    row.push_back(8);
    qq.push_back(row);
    $display("nested %0d %0d %0d %0d %0d", $size(qq[1]), $right(qq[1]), $high(qq[0]),
             $increment(qq[1]), $left(qq[0]));

    src.name = "bag";
    src.q = '{4, 5};
    w.id = 1;
    w.inner = src;
    src.q.push_back(6);
    w2 = w;
    w2.inner.q.push_back(7);
    $display("record-queue %s %0d %0d %0d %0d", w.inner.name, w.inner.q.size(), src.q.size(),
             w2.inner.q.size(), w2.inner.q[2]);
    $finish(0);
  end
endmodule

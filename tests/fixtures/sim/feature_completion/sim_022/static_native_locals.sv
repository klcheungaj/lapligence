// SIM-022-A01: static native locals of functions, tasks, blocks and class
// methods initialize once before time 0; automatic native locals initialize
// on every activation.
module tb;
  class Item;
    int n;
    function new(int v);
      n = v;
    endfunction
  endclass
  typedef struct { string s; int q[$]; } rec_t;

  function automatic string kinds(int k);
    static string s = "S";
    static int q[$] = '{1, 2};
    static int d[] = '{7, 8, 9};
    static int aa[string] = '{"a": 1};
    static Item item = new(5);
    static rec_t r = '{s: "r", q: '{3}};
    static semaphore sem = new(2);
    static mailbox #(int) mb = new;
    string as_ = "A";
    int aq[$] = '{1};
    Item ai = new(9);
    rec_t ar = '{s: "x", q: '{0}};
    s = {s, "s"};
    q.push_back(k);
    d[0] += k;
    aa[$sformatf("%0d", k)] = k;
    item.n += k;
    r.s = {r.s, "r"};
    r.q.push_back(k);
    void'(mb.try_put(k));
    as_ = {as_, "a"};
    aq.push_back(k);
    ai.n += k;
    ar.s = {ar.s, "x"};
    return $sformatf("s=%s q=%0d d0=%0d aa=%0d item=%0d r=%s/%0d sem=%0d mb=%0d | as=%s aq=%0d ai=%0d ar=%s",
                     s, q.size(), d[0], aa.num(), item.n, r.s, r.q.size(),
                     sem.try_get(1), mb.num(), as_, aq.size(), ai.n, ar.s);
  endfunction

  function static string static_function(int k);
    static string s = "T";
    static Item item = new(1);
    s = {s, "t"};
    item.n += k;
    return $sformatf("static_function s=%s item=%0d", s, item.n);
  endfunction

  task automatic waiting(int k);
    static string s = "w";
    static Item item = new(3);
    static rec_t r = '{s: "v", q: {}};
    string local_text = "z";
    #1;
    s = {s, "w"};
    item.n += k;
    r.q.push_back(k);
    local_text = {local_text, "z"};
    $display("waiting s=%s item=%0d r=%0d local=%s", s, item.n, r.q.size(), local_text);
  endtask

  class Owner;
    function int count(int k);
      static Item item = new(10);
      static string trail = "m";
      item.n += k;
      trail = {trail, "m"};
      $display("method trail=%s", trail);
      return item.n;
    endfunction
  endclass

  initial begin
    Owner owner;
    owner = new;
    $display("%s", kinds(1));
    $display("%s", kinds(2));
    $display("%s", static_function(1));
    $display("%s", static_function(2));
    waiting(1);
    waiting(2);
    $display("method item=%0d", owner.count(1));
    $display("method item=%0d", owner.count(2));
    for (int i = 0; i < 2; i++) begin
      static string kept = "k";
      static Item shared = new(0);
      static int sq[$] = '{9};
      automatic string again = "g";
      automatic int lq[$] = '{i};
      kept = {kept, "k"};
      shared.n++;
      sq.push_back(i);
      again = {again, "g"};
      lq.push_back(i);
      $display("loop kept=%s shared=%0d sq=%0d again=%s lq=%0d", kept, shared.n,
               sq.size(), again, lq.size());
    end
    $finish;
  end
endmodule

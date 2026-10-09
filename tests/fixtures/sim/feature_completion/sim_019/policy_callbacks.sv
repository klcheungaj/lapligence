// SIM-019 llg policy, not an IEEE 1800-2009 requirement: llg evaluates a
// `with` expression exactly once per element, in index or key order, and
// `unique`/`unique_index` keep the first occurrence of each value in index
// order. The standard leaves all of this open: "If the expression contained
// in the with clause includes any side effects, the results may be
// unpredictable" (7.12), "Array locator methods traverse the array in an
// unspecified order" (7.12.1), and for unique "The ordering of the returned
// elements is unrelated to the ordering of the original array. The index
// returned for duplicate valued entries may be the index for one of the
// duplicates" (7.12.1). The portable oracles are in `callbacks`,
// `locators_elements` and `index_keys`.
module tb;
  typedef struct {
    string name;
    int score;
  } entry_t;

  int calls;
  int trace[$];
  string order;

  function automatic int probe(int value);
    calls++;
    trace.push_back(value);
    return value;
  endfunction

  function automatic int note(string key);
    calls++;
    order = {order, key};
    return key.len();
  endfunction

  function automatic int count_above(int q[$], int limit);
    int hits[$];
    hits = q.find_index with (item > limit);
    return hits.size();
  endfunction

  function automatic int pick();
    calls++;
    return 1;
  endfunction

  int q[$] = '{4, 9, 2, 9, 7};
  int r[$];
  string s[$] = '{"bb", "a", "ccc"};
  int sa[string];
  entry_t e[$];
  int rows[2][3];
  int total;
  byte fd[4:1] = '{8'd4, 8'd2, 8'd4, 8'd9};
  int wi[$];

  initial begin
    calls = 0;
    r = q.find with (probe(item) > 5);
    $display("find %0d calls %0d trace %0d %0d %0d %0d %0d", r.size(), calls,
             trace[0], trace[1], trace[2], trace[3], trace[4]);

    calls = 0;
    trace.delete();
    total = q.sum() with (probe(item));
    $display("sum %0d calls %0d", total, calls);

    calls = 0;
    r = q.max() with (probe(item));
    $display("max %0d calls %0d", r[0], calls);

    calls = 0;
    r = q.find_first with (probe(item) == 9);
    $display("first %0d calls-ok %0d", r[0], calls >= 2 && calls <= 5);

    calls = 0;
    order = "";
    sa["zed"] = 1;
    sa["amy"] = 2;
    sa["kim"] = 3;
    total = sa.sum() with (note(item.index()));
    $display("assoc %0d calls %0d order %s", total, calls, order);

    calls = 0;
    order = "";
    s.sort() with (note(item));
    $display("sort %s %s %s calls-ok %0d", s[0], s[1], s[2], calls >= 3);

    for (int limit = 3; limit <= 8; limit += 5) begin
      automatic int bias = limit;
      r = q.find with (item + bias > 10);
      $display("limit %0d count %0d capture %0d", limit, count_above(q, limit), r.size());
    end

    e.push_back('{"x", 3});
    e.push_back('{"y", 8});
    e.push_back('{"z", 5});
    begin
      automatic int floor_score = 4;
      automatic entry_t picked[$];
      picked = e.find with (item.score > floor_score);
      e.sort() with (item.score * -1);
      $display("records %0d %s %s %s", picked.size(), e[0].name, e[1].name, e[2].name);
    end

    begin
      automatic string w[$] = '{"pear", "fig", "apple", "fig", "kiwi"};
      automatic string wu[$];
      wu = w.unique();
      wi = fd.unique_index();
      $display("unique %s %s %s %s index %0d %0d %0d", wu[0], wu[1], wu[2], wu[3], wi[0], wi[1],
               wi[2]);
    end

    calls = 0;
    rows[1] = '{1, 2, 3};
    rows[0] = '{7, 7, 7};
    total = rows[pick()].sum();
    $display("row %0d calls %0d", total, calls);
    $finish(0);
  end
endmodule

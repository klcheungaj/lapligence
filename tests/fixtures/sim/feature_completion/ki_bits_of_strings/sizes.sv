module tb;
  string s = "abc", e, n = "x";
  string list [$] = '{"a", "bcd"};
  string fixed [2] = '{"q", "rst"};
  bit flag = 1;
  string record;
  int total;
  class holder;
    string text = "held!";
  endclass
  holder h = new;
  typedef struct { string name; int id; } named_t;
  named_t item = '{"item", 1};

  initial begin
    $display("variable %0d %0d", $bits(s), $bits(e));
    $display("cast %0d", $bits(string'("xyz")));
    $display("concatenation %0d %0d", $bits({s, "de"}), $bits({e, n}));
    $display("replication %0d %0d", $bits({3{s}}), $bits({3{e}}));
    $display("conditional %0d %0d", $bits(flag ? s : n), $bits(s == "abc" ? "ab" : s));
    $display("elements %0d %0d", $bits(list[1]), $bits(fixed[1]));
    $display("property %0d member %0d", $bits(h.text), $bits(item.name));
    $display("typename %0d", $bits($typename(s)));
    $display("literal %0d", $bits("xyz"));
    s = {s, s};
    total = $bits(s) + $bits({s, n});
    $display("grown %0d", total);
    $display("method %0d", $bits(s.len()));
    $finish;
  end
endmodule

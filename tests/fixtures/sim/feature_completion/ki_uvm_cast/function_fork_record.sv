`timescale 1ns/1ns
// 9.3.2 and 6.21: a record local of a fork branch in a class function, with
// class queue, associative array and record properties read from the branch.
typedef struct {
  string id;
  int level;
  time offset;
} setting_t;
class c;
  setting_t q[$];
  int keys[string];
  setting_t last;
  function void go();
    for (int i = 0; i < q.size(); i++) begin
      automatic int j = i;
      fork begin
        automatic setting_t setting = q[j];
        #setting.offset;
        keys[setting.id] = setting.level;
        last = setting;
        if (setting.id == "_ALL_") $display("%0t all %0d", $time, setting.level);
        else $display("%0t %s %0d %0d", $time, setting.id, setting.level, keys.size());
      end join_none
    end
    fork begin
      automatic setting_t past = q[q.size()];
      $display("past [%s] %0d %b", past.id, past.level, past.offset[1:0]);
    end join_none
  endfunction
  function void shared();
    for (int i = 0; i < 2; i++) begin
      fork begin
        automatic int seen = i;
        automatic setting_t copy = q[seen - 1];
        #7 $display("%0t shared %0d %s", $time, seen, copy.id);
      end join_none
    end
  endfunction
endclass
function automatic void plain(int n);
  fork begin
    automatic setting_t held;
    held.id = "x";
    held.level = n;
    #2 $display("%0t plain %s %0d", $time, held.id, held.level);
  end join_none
endfunction
module tb;
  initial begin
    automatic c k = new;
    automatic setting_t s = '{"_ALL_", 1, 5};
    k.q.push_back(s);
    s = '{"x", 2, 3};
    k.q.push_back(s);
    k.go();
    k.shared();
    plain(2);
    #1 k.q[0].level = 9;
    #9 $display("%s %0d %0d", k.last.id, k.last.level, k.keys["x"]);
  end
endmodule

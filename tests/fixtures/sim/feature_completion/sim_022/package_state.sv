// SIM-022-A02: package variables, handles and static subroutine state are one
// copy shared by every importer and instance; names re-exported through a
// chain of packages denote the same storage.
package base;
  class Tally;
    int hits;
  endclass
  string text = "pkg";
  int numbers[$] = '{10};
  int table_[string];
  Tally tally = new;
  typedef struct { string s; int q[$]; } rec_t;
  rec_t rec = '{s: "rec", q: '{1}};
  function automatic void add(int k);
    static int calls = 0;
    static string trail = "c";
    calls++;
    trail = {trail, "c"};
    numbers.push_back(k);
    table_[$sformatf("%0d", k)] = calls;
    text = {text, "+"};
    tally.hits++;
    rec.q.push_back(k);
  endfunction
  function automatic string trail_of();
    static string trail = "t";
    trail = {trail, "t"};
    return trail;
  endfunction
endpackage

package middle;
  import base::*;
  export base::text;
  export base::add;
  string derived = {base::text, "_mid"};
endpackage

package outer;
  import middle::*;
  export *::*;
endpackage

module user;
  import outer::*;
  initial add(1);
endmodule

module tb;
  string first;
  string second;
  user u1();
  user u2();
  initial begin
    #1;
    base::add(5);
    $display("text=%s numbers=%0d table=%0d hits=%0d rec=%s/%0d",
             outer::text, base::numbers.size(), base::table_.num(),
             base::tally.hits, base::rec.s, base::rec.q.size());
    first = base::trail_of();
    second = base::trail_of();
    $display("derived=%s trail=%s trail=%s", middle::derived, first, second);
    $finish;
  end
endmodule

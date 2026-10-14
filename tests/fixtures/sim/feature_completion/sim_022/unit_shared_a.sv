// SIM-022-A02: compilation-unit declarations of the first file. Merged
// units share them with the second file; separate units keep them private.
class UnitCounter;
  int value;
  function new(int v);
    value = v;
  endfunction
endclass
string unit_text = "unitA";
int unit_queue[$] = '{1};
UnitCounter unit_counter = new(10);
function automatic void unit_add(int k);
  unit_queue.push_back(k);
  unit_text = {unit_text, "+"};
  unit_counter.value += k;
endfunction
module ma;
  initial begin
    unit_add(1);
    $display("ma text=%s queue=%0d counter=%0d", unit_text, unit_queue.size(), unit_counter.value);
  end
endmodule

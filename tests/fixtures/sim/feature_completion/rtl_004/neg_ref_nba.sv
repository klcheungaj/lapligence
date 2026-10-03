module tb; typedef int pair_t[2]; int a,b; task t(ref int x); pair_t'{x,b} <= '{1,2}; endtask initial t(a); endmodule

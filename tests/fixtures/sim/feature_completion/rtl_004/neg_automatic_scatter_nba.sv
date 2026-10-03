module tb; typedef int pair_t[2]; task automatic t(); int a,b; pair_t'{a,b} <= '{1,2}; endtask initial t(); endmodule

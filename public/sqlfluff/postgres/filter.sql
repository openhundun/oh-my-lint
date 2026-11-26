select
    count(*) filter(where c_expires > current_timestamp) as c_active
from t_test

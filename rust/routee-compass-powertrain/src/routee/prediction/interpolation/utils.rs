use std::cmp::Ordering;

use ordered_float::OrderedFloat;

pub fn linspace(x0: f64, xend: f64, n: usize) -> Vec<f64> {
    let dx = (xend - x0) / ((n - 1) as f64);
    let mut x = vec![x0; n];
    for i in 1..n {
        x[i] = x[i - 1] + dx;
    }
    x
}

fn find_nearest_index(arr: &[OrderedFloat<f64>], target: OrderedFloat<f64>) -> usize {
    let mut low = 0;
    let mut high = arr.len() - 1;

    while low < high {
        let mid = low + (high - low) / 2;

        if arr[mid] >= target {
            high = mid;
        } else {
            low = mid + 1;
        }
    }

    if low > 0 && arr[low] >= target {
        low - 1
    } else {
        low
    }
}

pub struct BilinearInterp {
    pub x: Vec<OrderedFloat<f64>>,
    pub y: Vec<OrderedFloat<f64>>,
    pub values: Vec<Vec<f64>>,
}

impl BilinearInterp {
    pub fn new(x: Vec<f64>, y: Vec<f64>, values: Vec<Vec<f64>>) -> Result<Self, String> {
        if x.len() != values.len() {
            return Err("Supplied `x` must have same dimensionality as `values`".to_string());
        }
        if y.len() != values[0].len() {
            return Err("Supplied `y` must have same dimensionality as `values`".to_string());
        }
        if !x.windows(2).all(|w| w[0] < w[1]) {
            return Err("Supplied `x` coordinates must be sorted and non-repeating".to_string());
        }
        if !y.windows(2).all(|w| w[0] < w[1]) {
            return Err("Supplied `y` coordinates must be sorted and non-repeating".to_string());
        }
        let x = x.into_iter().map(OrderedFloat::from).collect();
        let y = y.into_iter().map(OrderedFloat::from).collect();
        Ok(BilinearInterp { x, y, values })
    }

    pub fn interpolate(&self, x: f64, y: f64) -> Result<f64, &'static str> {
        let x_index = find_nearest_index(&self.x, OrderedFloat(x));
        let y_index = find_nearest_index(&self.y, OrderedFloat(y));

        if x_index >= self.x.len() - 1 || y_index >= self.y.len() - 1 {
            return Err("Cannot interpolate outside of grid bounds");
        }
        let x0 = self.x[x_index].into_inner();
        let x1 = self.x[x_index + 1].into_inner();
        let y0 = self.y[y_index].into_inner();
        let y1 = self.y[y_index + 1].into_inner();

        let q11 = self.values[x_index][y_index];
        let q12 = self.values[x_index][y_index + 1];
        let q21 = self.values[x_index + 1][y_index];
        let q22 = self.values[x_index + 1][y_index + 1];

        let fxy1 = (x1 - x) / (x1 - x0) * q11 + (x - x0) / (x1 - x0) * q21;
        let fxy2 = (x1 - x) / (x1 - x0) * q12 + (x - x0) / (x1 - x0) * q22;

        Ok((y1 - y) / (y1 - y0) * fxy1 + (y - y0) / (y1 - y0) * fxy2)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // test targets found using https://www.omnicalculator.com/math/bilinear-interpolation
    #[test]
    fn test_multilinear_2d() {
        let x = vec![0.0, 1.0, 2.0];
        let y = vec![0.0, 1.0, 2.0];
        let values = vec![
            vec![0.0, 2.0, 1.9], // (x0, y0), (x0, y1), (x0, y2)
            vec![2.0, 4.0, 3.1], // (x1, y0), (x1, y1), (x1, y2)
            vec![5.0, 0.0, 1.4], // (x2, y0), (x2, y1), (x2, y2)
        ];

        let interp = BilinearInterp::new(x, y, values.clone()).unwrap();

        assert_eq!(interp.interpolate(0.5, 0.5).unwrap(), 2.0);

        assert_eq!(interp.interpolate(1.52, 0.36).unwrap(), 2.9696);

        // returns value at (x2, y2)
        assert_eq!(interp.interpolate(2.0, 2.0).unwrap(), values[2][2]);

        // errors out for values greater than bounds 
        assert!(interp.interpolate(3.0, 3.0).is_err());
        
    }
}
